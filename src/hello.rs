use crate::uring::*;

pub struct HelloCommand {
    pub msg: &'static [u8],
}

pub enum HelloCommandExecute {
    Succeeded(),
    Failed(&'static [u8]),
}

impl IORingSubmitBuffer for &'static [u8] {
    fn extract(self) -> (*const u8, usize) {
        (self.as_ptr(), self.len())
    }
}

fn fail(msg: &'static [u8]) -> HelloCommandExecute {
    HelloCommandExecute::Failed(msg)
}

impl HelloCommand {
    const IORING_INVALID_DESCRIPTOR: &'static [u8] = b"I/O Ring Init failed: Invalid Descriptor.\n";
    const IORING_SETUP_FAILED: &'static [u8] = b"I/O Ring Init failed: Setup Failed.\n";
    const IORING_MAPPING_FAILED: &'static [u8] = b"I/O Ring Init failed: Mapping Failed.\n";
    const IORING_SUBMISSION_FAILED: &'static [u8] = b"I/O Ring entry submission failed.\n";
    const IORING_SUBMISSION_MISMATCHED: &'static [u8] = b"I/O Ring entry submission mismatch.\n";
    const IORING_COMPLETION_FAILED: &'static [u8] = b"I/O Ring entry completion failed.\n";
    const IORING_COMPLETION_ERRORED: &'static [u8] = b"I/O Ring completed with failure.\n";
    const IORING_SHUTDOWN_FAILED: &'static [u8] = b"I/O Ring shutdown failed.\n";
}

impl HelloCommand {
    pub fn execute(&self) -> HelloCommandExecute {
        let mut ring = match IORing::init(32) {
            IORingInit::Succeeded(value) => value,
            IORingInit::InvalidDescriptor(_) => return fail(HelloCommand::IORING_INVALID_DESCRIPTOR),
            IORingInit::SetupFailed(_) => return fail(HelloCommand::IORING_SETUP_FAILED),
            IORingInit::MappingFailed(_, _) => return fail(HelloCommand::IORING_MAPPING_FAILED),
        };

        let op = IORingSubmitEntry::write(2, self.msg, 0, 0);

        match ring.submit([op]) {
            IORingSubmit::SubmissionFailed(_) => return fail(HelloCommand::IORING_SUBMISSION_FAILED),
            IORingSubmit::SubmissionMismatched(_) => return fail(HelloCommand::IORING_SUBMISSION_MISMATCHED),
            IORingSubmit::Succeeded(_) => (),
        };

        let entry = loop {
            match ring.complete() {
                IORingComplete::Succeeded(entry) => break entry,
                IORingComplete::UnexpectedEmpty(_) => continue,
                IORingComplete::CompletionFailed(_) => return fail(HelloCommand::IORING_COMPLETION_FAILED),
            }
        };

        if entry.res < 0 {
            return HelloCommandExecute::Failed(HelloCommand::IORING_COMPLETION_ERRORED);
        }

        if let IORingShutdown::Failed() = ring.shutdown() {
            return fail(HelloCommand::IORING_SHUTDOWN_FAILED);
        }

        HelloCommandExecute::Succeeded()
    }
}
