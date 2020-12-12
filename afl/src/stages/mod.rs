pub mod mutational;
pub use mutational::StdMutationalStage;

use crate::corpus::Corpus;
use crate::engines::{Engine, State};
use crate::events::EventManager;
use crate::executors::{Executor, ExecutorsTuple, HasObservers};
use crate::feedbacks::FeedbacksTuple;
use crate::inputs::Input;
use crate::observers::ObserversTuple;
use crate::tuples::TupleList;
use crate::utils::Rand;
use crate::AflError;

/// A stage is one step in the fuzzing process.
/// Multiple stages will be scheduled one by one for each input.
pub trait Stage<EM, E, OT, FT, ET, C, I, R>
where
    EM: EventManager<C, E, OT, FT, I, R>,
    E: Executor<I> + HasObservers<OT>,
    OT: ObserversTuple,
    FT: FeedbacksTuple<I>,
    ET: ExecutorsTuple<I>,
    C: Corpus<I, R>,
    I: Input,
    R: Rand,
{
    /// Run the stage
    fn perform(
        &mut self,
        rand: &mut R,
        state: &mut State<I, R, FT>,
        corpus: &mut C,
        engine: &mut Engine<E, OT, ET, I>,
        manager: &mut EM,
        corpus_idx: usize,
    ) -> Result<(), AflError>;
}

pub trait StagesTuple<EM, E, OT, FT, ET, C, I, R>
where
    EM: EventManager<C, E, OT, FT, I, R>,
    E: Executor<I> + HasObservers<OT>,
    OT: ObserversTuple,
    FT: FeedbacksTuple<I>,
    ET: ExecutorsTuple<I>,
    C: Corpus<I, R>,
    I: Input,
    R: Rand,
{
    fn perform_all(
        &mut self,
        rand: &mut R,
        state: &mut State<I, R, FT>,
        corpus: &mut C,
        engine: &mut Engine<E, OT, ET, I>,
        manager: &mut EM,
        corpus_idx: usize,
    ) -> Result<(), AflError>;
    fn for_each(&self, f: fn(&dyn Stage<EM, E, OT, FT, ET, C, I, R>));
    fn for_each_mut(&mut self, f: fn(&mut dyn Stage<EM, E, OT, FT, ET, C, I, R>));
}

impl<EM, E, OT, FT, ET, C, I, R> StagesTuple<EM, E, OT, FT, ET, C, I, R> for ()
where
    EM: EventManager<C, E, OT, FT, I, R>,
    E: Executor<I> + HasObservers<OT>,
    OT: ObserversTuple,
    FT: FeedbacksTuple<I>,
    ET: ExecutorsTuple<I>,
    C: Corpus<I, R>,
    I: Input,
    R: Rand,
{
    fn perform_all(
        &mut self,
        _rand: &mut R,
        _state: &mut State<I, R, FT>,
        _corpus: &mut C,
        _engine: &mut Engine<E, OT, ET, I>,
        _manager: &mut EM,
        _corpus_idx: usize,
    ) -> Result<(), AflError> {
        Ok(())
    }
    fn for_each(&self, _f: fn(&dyn Stage<EM, E, OT, FT, ET, C, I, R>)) {}
    fn for_each_mut(&mut self, _f: fn(&mut dyn Stage<EM, E, OT, FT, ET, C, I, R>)) {}
}

impl<Head, Tail, EM, E, OT, FT, ET, C, I, R> StagesTuple<EM, E, OT, FT, ET, C, I, R>
    for (Head, Tail)
where
    Head: Stage<EM, E, OT, FT, ET, C, I, R>,
    Tail: StagesTuple<EM, E, OT, FT, ET, C, I, R> + TupleList,
    EM: EventManager<C, E, OT, FT, I, R>,
    E: Executor<I> + HasObservers<OT>,
    OT: ObserversTuple,
    FT: FeedbacksTuple<I>,
    ET: ExecutorsTuple<I>,
    C: Corpus<I, R>,
    I: Input,
    R: Rand,
{
    fn perform_all(
        &mut self,
        rand: &mut R,
        state: &mut State<I, R, FT>,
        corpus: &mut C,
        engine: &mut Engine<E, OT, ET, I>,
        manager: &mut EM,
        corpus_idx: usize,
    ) -> Result<(), AflError> {
        self.0
            .perform(rand, state, corpus, engine, manager, corpus_idx)?;
        self.1
            .perform_all(rand, state, corpus, engine, manager, corpus_idx)
    }

    fn for_each(&self, f: fn(&dyn Stage<EM, E, OT, FT, ET, C, I, R>)) {
        f(&self.0);
        self.1.for_each(f)
    }

    fn for_each_mut(&mut self, f: fn(&mut dyn Stage<EM, E, OT, FT, ET, C, I, R>)) {
        f(&mut self.0);
        self.1.for_each_mut(f)
    }
}