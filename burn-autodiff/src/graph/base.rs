use spin::Mutex;
use std::{collections::HashMap, sync::Arc};

use crate::{
    checkpoint::{base::Checkpointer, builder::build_checkpointer},
    grads::Gradients,
    ops::CheckpointingAction,
};

use super::{NodeID, NodeRef};

/// Backward step for reverse mode autodiff.
pub trait Step: Send + Sync + std::fmt::Debug {
    /// Executes the step and consumes it.
    fn step(self: Box<Self>, grads: &mut Gradients, checkpointer: &mut Checkpointer);
    /// The node associated to the step.
    fn node(&self) -> NodeRef;
}

pub type StepBoxed = Box<dyn Step>;
pub type NodeSteps = HashMap<NodeID, StepBoxed>;

#[derive(new, Debug, Default)]
pub struct CheckpointingActions {
    pub main_actions: Vec<CheckpointingAction>,
    pub backup_actions: Vec<CheckpointingAction>,
}

impl CheckpointingActions {
    fn extend(&mut self, other: CheckpointingActions) {
        for other_action in other.main_actions {
            self.main_actions.push(other_action)
        }
        for other_unsure in other.backup_actions {
            self.backup_actions.push(other_unsure)
        }
    }

    fn len(&self) -> usize {
        self.main_actions.len() + self.backup_actions.len()
    }
}

/// Graph data structure.
///
/// The graph contains the [node steps](Step), which can be access by [node id](NodeID).
#[derive(Default, Clone, Debug)]
pub struct Graph {
    steps: Arc<Mutex<NodeSteps>>,
    checkpointing_actions: Arc<Mutex<CheckpointingActions>>,
}

impl Graph {
    /// Create a new graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get all the steps for the graph.
    ///
    /// # Notes
    ///
    /// This is a owned method, so the current graph will be freed. However, the steps can
    /// be shared with other graphs, therefore they are going to be cleared.
    ///
    /// This is useful, since the graph is supposed to be consumed only once for backprop, and
    /// keeping all the tensors alive for multiple backward call is a heavy waste of resources.
    pub fn steps(self) -> NodeSteps {
        let mut map_drain = HashMap::new();
        self.execute_mut_steps(|map| {
            std::mem::swap(&mut *map, &mut map_drain);
        });
        map_drain
    }

    /// Register a new step into the graph.
    pub fn register(self, id: &NodeID, ops: StepBoxed) -> Self {
        self.execute_mut_steps(|map| {
            map.insert(id.clone(), ops);
        })
    }

    /// Merge two graphs.
    pub fn merge(self, other: Self) -> Self {
        if Arc::ptr_eq(&self.steps, &other.steps) {
            return self;
        }

        self.merge_different(other)
    }

    fn execute_mut_steps<F: FnOnce(&mut NodeSteps)>(mut self, func: F) -> Self {
        match Arc::get_mut(&mut self.steps) {
            Some(mutex) => {
                let map = mutex.get_mut();
                func(map);
            }
            None => {
                // Only lock when there are multiple references to the graph.
                let mut map = self.steps.lock();
                func(&mut map);
            }
        };

        self
    }

    fn merge_different(self, other: Self) -> Self {
        let mut map2 = other.clone().steps();
        let mut actions2 = other.checkpointing_actions_own();

        self.execute_mut_steps(|map1| {
            if map1.len() > map2.len() {
                map1.extend(map2);
            } else {
                let mut map_drain = HashMap::new();
                std::mem::swap(map1, &mut map_drain);
                map2.extend(map_drain);
                std::mem::swap(map1, &mut map2);
            }
        })
        .execute_mut_checkpointing_actions(|actions1| {
            if actions1.len() > actions2.len() {
                actions1.extend(actions2);
            } else {
                let mut checkpointing_drain = CheckpointingActions::default();
                std::mem::swap(actions1, &mut checkpointing_drain);
                actions2.extend(checkpointing_drain);
                std::mem::swap(actions1, &mut actions2);
            }
        })
    }

    /// # Notes
    ///
    /// This is a owned method, so the current checkpointer will be freed.
    pub fn checkpointing_actions_own(self) -> CheckpointingActions {
        let mut actions = CheckpointingActions::default();
        self.execute_mut_checkpointing_actions(|checkpointing_actions| {
            std::mem::swap(&mut *checkpointing_actions, &mut actions);
        });
        actions
    }

    fn execute_mut_checkpointing_actions<F: FnOnce(&mut CheckpointingActions)>(
        mut self,
        func: F,
    ) -> Self {
        match Arc::get_mut(&mut self.checkpointing_actions) {
            Some(mutex) => {
                let map = mutex.get_mut();
                func(map);
            }
            None => {
                // Only lock when there are multiple references to the graph.
                let mut actions = self.checkpointing_actions.lock();
                func(&mut actions);
            }
        };

        self
    }

    pub(crate) fn build_checkpointer(&self) -> Checkpointer {
        let mut guard = self.checkpointing_actions.lock();
        let owned: CheckpointingActions =
            std::mem::replace(&mut *guard, CheckpointingActions::default());
        build_checkpointer(owned.main_actions, owned.backup_actions, &self.steps.lock())
    }

    pub(crate) fn extend_checkpointing_actions(&self, checkpointing_actions: CheckpointingActions) {
        self.checkpointing_actions
            .lock()
            .extend(checkpointing_actions);
    }
}
