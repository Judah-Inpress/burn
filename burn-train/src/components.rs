use crate::{
    checkpoint::{Checkpointer, CheckpointingStrategy},
    metric::processor::EventProcessor,
};
use burn_core::{
    lr_scheduler::LrScheduler,
    module::{ADModule, Module},
    optim::Optimizer,
    tensor::backend::ADBackend,
};
use std::marker::PhantomData;

/// All components necessary to train a model grouped in one trait.
pub trait LearnerComponents {
    /// The backend in used for the training.
    type Backend: ADBackend;
    /// The learning rate scheduler used for the training.
    type LrScheduler: LrScheduler;
    /// The model to train.
    type Model: ADModule<Self::Backend> + core::fmt::Display + 'static;
    /// The optimizer used for the training.
    type Optimizer: Optimizer<Self::Model, Self::Backend>;
    /// The checkpointer used for the model.
    type CheckpointerModel: Checkpointer<<Self::Model as Module<Self::Backend>>::Record>;
    /// The checkpointer used for the optimizer.
    type CheckpointerOptimizer: Checkpointer<
        <Self::Optimizer as Optimizer<Self::Model, Self::Backend>>::Record,
    >;
    /// The checkpointer used for the scheduler.
    type CheckpointerLrScheduler: Checkpointer<<Self::LrScheduler as LrScheduler>::Record>;
    type EventProcessor: EventProcessor + 'static;
    /// The strategy to save and delete checkpoints.
    type CheckpointerStrategy: CheckpointingStrategy;
}

/// Concrete type that implements [training components trait](TrainingComponents).
pub struct LearnerComponentsMarker<B, LR, M, O, CM, CO, CS, EP, S> {
    _backend: PhantomData<B>,
    _lr_scheduler: PhantomData<LR>,
    _model: PhantomData<M>,
    _optimizer: PhantomData<O>,
    _checkpointer_model: PhantomData<CM>,
    _checkpointer_optim: PhantomData<CO>,
    _checkpointer_scheduler: PhantomData<CS>,
    _event_processor: PhantomData<EP>,
    _strategy: S,
}

impl<B, LR, M, O, CM, CO, CS, EP, S> LearnerComponents
    for LearnerComponentsMarker<B, LR, M, O, CM, CO, CS, EP, S>
where
    B: ADBackend,
    LR: LrScheduler,
    M: ADModule<B> + core::fmt::Display + 'static,
    O: Optimizer<M, B>,
    CM: Checkpointer<M::Record>,
    CO: Checkpointer<O::Record>,
    CS: Checkpointer<LR::Record>,
    EP: EventProcessor + 'static,
    S: CheckpointingStrategy,
{
    type Backend = B;
    type LrScheduler = LR;
    type Model = M;
    type Optimizer = O;
    type CheckpointerModel = CM;
    type CheckpointerOptimizer = CO;
    type CheckpointerLrScheduler = CS;
    type EventProcessor = EP;
    type CheckpointerStrategy = S;
}