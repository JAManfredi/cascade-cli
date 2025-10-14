use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

/// Lightweight wrapper around `indicatif`'s spinner progress bar with
/// convenience helpers for printing output while the spinner is active.
pub struct Spinner {
    pb: ProgressBar,
}

/// Cloneable handle that allows printing while a spinner is active.
#[derive(Debug, Clone)]
pub struct SpinnerPrinter {
    pb: ProgressBar,
}

impl Spinner {
    const TICK_RATE: Duration = Duration::from_millis(80);
    const TEMPLATE: &'static str = "{spinner:.green} {msg}";

    fn new_internal(message: String) -> Self {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::with_template(Self::TEMPLATE)
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        pb.set_message(message);
        pb.enable_steady_tick(Self::TICK_RATE);
        Spinner { pb }
    }

    /// Start a spinner with the provided message.
    pub fn new(message: String) -> Self {
        Self::new_internal(message)
    }

    /// Start a spinner intended to have output printed underneath it.
    ///
    /// (This currently behaves the same as `new`, but exists to preserve the
    /// semantics of the previous implementation and allow future tweaks.)
    pub fn new_with_output_below(message: String) -> Self {
        Self::new_internal(message)
    }

    /// Print a line while keeping the spinner intact.
    pub fn println<T: AsRef<str>>(&self, message: T) {
        self.pb.println(message.as_ref());
    }

    /// Obtain a cloneable printer handle that can be used to emit lines from
    /// other parts of the code while the spinner remains active.
    pub fn printer(&self) -> SpinnerPrinter {
        SpinnerPrinter {
            pb: self.pb.clone(),
        }
    }

    /// Temporarily suspend the spinner while executing the provided closure.
    pub fn suspend<F: FnOnce()>(&self, f: F) {
        self.pb.suspend(f);
    }

    /// Stop the spinner and clear it from the terminal.
    pub fn stop(&self) {
        self.pb.finish_and_clear();
    }

    /// Stop the spinner and replace it with a final message.
    pub fn stop_with_message(&self, message: &str) {
        self.pb.finish_with_message(message.to_string());
    }

    /// Update the spinner message while it is running.
    pub fn update_message(&self, new_message: String) {
        self.pb.set_message(new_message);
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        if !self.pb.is_finished() {
            self.pb.finish_and_clear();
        }
    }
}

impl SpinnerPrinter {
    /// Print a line beneath the spinner.
    pub fn println<T: AsRef<str>>(&self, message: T) {
        self.pb.println(message.as_ref());
    }

    /// Temporarily suspend the spinner while running the provided closure.
    pub fn suspend<F: FnOnce()>(&self, f: F) {
        self.pb.suspend(f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_spinner_creation_and_stop() {
        let spinner = Spinner::new("Testing".to_string());
        thread::sleep(Duration::from_millis(200));
        spinner.stop();
    }

    #[test]
    fn test_spinner_with_message() {
        let spinner = Spinner::new("Loading".to_string());
        thread::sleep(Duration::from_millis(200));
        spinner.stop_with_message("✓ Done");
    }
}
