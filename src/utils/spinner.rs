use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// A simple terminal spinner that runs in a background thread
///
/// The spinner automatically stops and cleans up when dropped, making it
/// safe to use in code paths that may error or panic.
///
/// # Example
/// ```no_run
/// use cascade_cli::utils::spinner::Spinner;
///
/// let mut spinner = Spinner::new("Loading data".to_string());
/// // ... do some work ...
/// spinner.stop_with_message("✓ Data loaded");
/// ```
pub struct Spinner {
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
    message: String,
}

impl Spinner {
    /// Braille spinner frames for smooth animation
    const FRAMES: &'static [&'static str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

    /// Frame duration in milliseconds
    const FRAME_DURATION_MS: u64 = 80;

    /// Start a new spinner with the given message
    ///
    /// The spinner will animate in a background thread until stopped.
    pub fn new(message: String) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        let message_clone = message.clone();

        let handle = thread::spawn(move || {
            let mut frame_idx = 0;
            while running_clone.load(Ordering::Relaxed) {
                let frame = Self::FRAMES[frame_idx % Self::FRAMES.len()];
                print!("\r{} {}...", frame, message_clone);
                io::stdout().flush().ok();

                frame_idx += 1;
                thread::sleep(Duration::from_millis(Self::FRAME_DURATION_MS));
            }
        });

        Spinner {
            running,
            handle: Some(handle),
            message,
        }
    }

    /// Start a new spinner that stays on one line while content appears below
    ///
    /// This variant prints the message with a newline, then updates only the
    /// spinner character using ANSI cursor positioning. Content can be printed
    /// below without being overwritten by the spinner.
    pub fn new_with_output_below(message: String) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);
        let message_clone = message.clone();

        // Print initial message with newline
        println!("{} {}...", Self::FRAMES[0], message_clone);
        io::stdout().flush().ok();

        let handle = thread::spawn(move || {
            let mut frame_idx = 1; // Start at 1 since we already printed frame 0
            while running_clone.load(Ordering::Relaxed) {
                let frame = Self::FRAMES[frame_idx % Self::FRAMES.len()];

                // Move cursor up 1 line, go to column 0, print spinner, move cursor down 1 line
                // This updates just the spinner character without touching content below
                print!("\x1b[1A\x1b[0G{}\x1b[1B\x1b[0G", frame);
                io::stdout().flush().ok();

                frame_idx += 1;
                thread::sleep(Duration::from_millis(Self::FRAME_DURATION_MS));
            }
        });

        Spinner {
            running,
            handle: Some(handle),
            message,
        }
    }

    /// Stop the spinner and clear the line
    ///
    /// This method is idempotent - it's safe to call multiple times.
    pub fn stop(&mut self) {
        if !self.running.load(Ordering::Relaxed) {
            return; // Already stopped
        }

        self.running.store(false, Ordering::Relaxed);

        if let Some(handle) = self.handle.take() {
            handle.join().ok();
        }

        // Clear the spinner line
        self.clear_line();
    }

    /// Stop the spinner and replace with a final message
    ///
    /// This is useful for showing success/failure status after completion.
    pub fn stop_with_message(&mut self, message: &str) {
        if !self.running.load(Ordering::Relaxed) {
            return; // Already stopped
        }

        self.running.store(false, Ordering::Relaxed);

        if let Some(handle) = self.handle.take() {
            handle.join().ok();
        }

        // Clear line and print final message
        self.clear_line();
        println!("{}", message);
    }

    /// Update the spinner's message while it's running
    ///
    /// Note: This creates a brief flicker as we stop and restart the spinner.
    /// For frequent updates, consider using stop_with_message() instead.
    pub fn update_message(&mut self, new_message: String) {
        self.stop();
        *self = Self::new(new_message);
    }

    /// Clear the current line in the terminal
    fn clear_line(&self) {
        // Calculate clear width: spinner (1) + space (1) + message + ellipsis (3) + padding (5)
        let clear_width = self.message.len() + 10;
        print!("\r{}\r", " ".repeat(clear_width));
        io::stdout().flush().ok();
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        // Ensure spinner is stopped when dropped (e.g., on panic or early return)
        // This prevents orphaned spinner threads and terminal artifacts
        if self.running.load(Ordering::Relaxed) {
            self.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_creation_and_stop() {
        let mut spinner = Spinner::new("Testing".to_string());
        thread::sleep(Duration::from_millis(200));
        spinner.stop();
    }

    #[test]
    fn test_spinner_with_message() {
        let mut spinner = Spinner::new("Loading".to_string());
        thread::sleep(Duration::from_millis(200));
        spinner.stop_with_message("✓ Done");
    }
}
