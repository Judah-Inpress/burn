use std::collections::HashMap;

use crate::graph::{NodeID, NodeRef};

use super::state::State;

/// Definition of the forward function of a node, called during retropropagation only.
/// This is different from the normal forward function because it reads and writes from
/// the [InnerStates] map instead of having a clear function signature.
pub(crate) trait RetroForward {
    fn forward(&self, states: &mut InnerStates);
}

#[derive(new, Default)]
/// Links [NodeID]s to their corresponding [RetroForward]
pub(crate) struct RetroForwards {
    map: HashMap<NodeID, Box<dyn RetroForward>>,
}

impl RetroForwards {
    /// Executes the [RetroForward] for a given [NodeID] if the node's
    /// [State] is [State::Recompute], otherwise does nothing.
    pub fn forward(&self, node_id: &NodeID, inner_states: &mut InnerStates) {
        if let State::Recompute { n_required: _ } = inner_states.get_ref(node_id).unwrap() {
            self.map.get(&node_id).unwrap().forward(inner_states);
        }
    }

    /// Associates a [RetroForward] to its [NodeID]
    pub fn insert(&mut self, node_id: NodeID, retro_forward: Box<dyn RetroForward>) {
        self.map.insert(node_id, retro_forward);
    }
}

#[derive(new, Default)]
/// Links [NodeID]s to their current [State]
pub(crate) struct InnerStates {
    map: HashMap<NodeID, State>,
}

impl InnerStates {
    /// Returns the output in the [State] of the given [NodeID],
    /// and decrements the number of times this state is required.
    /// This function always gives ownership of the output, but will clone it if needed for further uses.
    pub fn get_owned_and_downcasted<T>(&mut self, node_id: &NodeID) -> T
    where
        T: Clone + Send + Sync + 'static,
    {
        // Fetch the state and decrement its number of required
        let state = self.map.remove(node_id).unwrap();
        let remaining_n_required = state.n_required() - 1;

        // Downcast the state to whatever it is supposed to be
        let downcasted = state
            .get_state_content()
            .downcast_ref::<T>()
            .unwrap()
            .clone();

        // If still needed after giving ownership, we copy it back to the hashmap
        if remaining_n_required > 0 {
            let new_stored_state = match state {
                State::Recompute { n_required: _ } => State::Recompute {
                    n_required: remaining_n_required,
                },
                State::Computed {
                    state_content: _,
                    n_required: _,
                } => State::Computed {
                    state_content: Box::new(downcasted.clone()),
                    n_required: remaining_n_required,
                },
            };

            self.insert(node_id.clone(), new_stored_state);
        }

        downcasted
    }

    /// Returns a reference to the [State] of the given node
    /// Useful when we need [State] information without needing the underlying tensor
    pub fn get_ref(&self, node_id: &NodeID) -> Option<&State> {
        self.map.get(node_id)
    }

    /// Associates a [State] to its [NodeID]
    pub fn insert(&mut self, node_id: NodeID, state: State) {
        self.map.insert(node_id, state);
    }
}

#[derive(new, Default)]
/// Links a [NodeID] to its autodiff graph [NodeRef]
pub(crate) struct NodeTree {
    map: HashMap<NodeID, NodeRef>,
}

impl NodeTree {
    /// Gives the parents of the node in the autodiff graph
    pub fn parents(&self, node_id: &NodeID) -> Vec<NodeID> {
        self.map.get(node_id).unwrap().parents.clone()
    }

    // Associates a [NodeRef] to its [NodeID]
    pub fn insert(&mut self, node_id: NodeID, node_ref: NodeRef) {
        self.map.insert(node_id, node_ref);
    }
}

#[derive(new)]
/// Struct responsible of fetching the output for a node in the autodiff graph during a backward pass
pub struct Checkpoint {
    inner_states: InnerStates,
    retro_forwards: RetroForwards,
    node_tree: NodeTree,
}

impl Checkpoint {
    /// Gives the output of the given node, by recursively asking parents to compute themselves
    /// or give their pre-computed tensors.
    pub fn get<T>(&mut self, node_id: NodeID) -> T
    where
        T: Clone + Send + Sync + 'static,
    {
        self.topological_sort(node_id.clone())
            .iter()
            .for_each(|node| self.retro_forwards.forward(&node, &mut self.inner_states));

        self.inner_states.get_owned_and_downcasted::<T>(&node_id)
    }

    /// Insert a [State::Precomputed] at [NodeID]
    /// This is the actual checkpointing
    pub fn insert_pre_computed(&mut self, node_id: NodeID, state: State) {
        if let State::Computed {
            state_content: _,
            n_required: _,
        } = state
        {
            self.inner_states.insert(node_id, state);
        } else {
            panic!("Can't insert Recompute state manually")
        }
    }

    /// Sorts the ancestors of NodeID in a way such that all parents come before their children
    /// Useful to avoid recursivity later when mutating the states
    fn topological_sort(&self, node_id: NodeID) -> Vec<NodeID> {
        match self.inner_states.get_ref(&node_id) {
            Some(state) =>
            {
                match state {
                State::Recompute {
                    n_required: _,
                } => {
                    let mut sorted = Vec::new();
                    for parent_node in self.node_tree.parents(&node_id) {
                        sorted.extend(self.topological_sort(parent_node));
                    }
                    sorted.push(node_id);
                    sorted
                }
                State::Computed {
                    state_content: _,
                    n_required: _,
                } => vec![node_id],
            }}
            None => panic!("Node is not in the map. You may have tried to access it more times than n_required allowed.")
        }
    }
}
