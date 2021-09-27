
pub trait EvaluationHook<I, S>
where
    I: Input,
{
    fn pre_evaluation(&mut self, state: &mut S, manager: &mut EM,
        input: &I, observers: &OT, exit_kind: &ExitKind);

    fn post_evaluation(
        &mut self,
        state: &mut S,
        testcase: Option<&mut Testcase<I>>,
    ) -> Result<(), Error>;
}

