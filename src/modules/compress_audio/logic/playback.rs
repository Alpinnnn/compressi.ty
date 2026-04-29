use std::{
    fs::File,
    path::{Path, PathBuf},
    time::Duration,
};

use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player};

/// Lightweight local audio preview player used by the details panel.
pub struct AudioPreviewPlayer {
    device_sink: Option<MixerDeviceSink>,
    player: Option<Player>,
    track_id: Option<u64>,
    track_path: Option<PathBuf>,
    last_error: Option<String>,
}

impl Default for AudioPreviewPlayer {
    fn default() -> Self {
        Self {
            device_sink: None,
            player: None,
            track_id: None,
            track_path: None,
            last_error: None,
        }
    }
}

impl AudioPreviewPlayer {
    /// Stops playback when the selected track changes or disappears.
    pub fn sync_selected_track(&mut self, selected_track_id: Option<u64>) {
        if self.track_id != selected_track_id {
            self.stop();
        }
    }

    /// Toggles playback for the current track, starting it when needed.
    pub fn toggle_playback(&mut self, track_id: u64, path: &Path) {
        self.refresh_finished_sink();
        if self.needs_reload(track_id, path) {
            if let Err(error) = self.load_track(track_id, path, false) {
                self.last_error = Some(error);
            }
            return;
        }

        let Some(player) = self.player.as_ref() else {
            return;
        };

        if player.is_paused() {
            player.play();
        } else {
            player.pause();
        }
        self.last_error = None;
    }

    /// Seeks the current track to a new position, lazily loading it if necessary.
    pub fn seek_to(&mut self, track_id: u64, path: &Path, position: Duration) {
        self.refresh_finished_sink();
        let resume_after_seek = self.is_playing();

        if self.needs_reload(track_id, path)
            && let Err(error) = self.load_track(track_id, path, true)
        {
            self.last_error = Some(error);
            return;
        }

        let Some(player) = self.player.as_ref() else {
            return;
        };

        if let Err(error) = player.try_seek(position) {
            self.last_error = Some(format!("Could not seek preview audio: {error}"));
            return;
        }

        if resume_after_seek {
            player.play();
        } else {
            player.pause();
        }
        self.last_error = None;
    }

    /// Returns the current playback position for the active track.
    pub fn playback_position(&mut self, total_duration: Duration) -> Duration {
        self.refresh_finished_sink();
        self.player
            .as_ref()
            .map(|player| player.get_pos().min(total_duration))
            .unwrap_or_default()
    }

    /// Returns whether the active preview is currently playing.
    pub fn is_playing(&mut self) -> bool {
        self.refresh_finished_sink();
        self.player
            .as_ref()
            .map(|player| !player.is_paused() && !player.empty())
            .unwrap_or(false)
    }

    /// Returns the latest preview playback error, if any.
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// Fully stops and clears the current preview session.
    pub fn stop(&mut self) {
        if let Some(player) = self.player.take() {
            player.stop();
        }
        self.device_sink = None;
        self.track_id = None;
        self.track_path = None;
        self.last_error = None;
    }

    fn needs_reload(&self, track_id: u64, path: &Path) -> bool {
        self.track_id != Some(track_id)
            || self.track_path.as_deref() != Some(path)
            || self.player.is_none()
    }

    fn load_track(&mut self, track_id: u64, path: &Path, start_paused: bool) -> Result<(), String> {
        self.stop();

        let mut device_sink = DeviceSinkBuilder::open_default_sink()
            .map_err(|error| format!("Could not open the default audio output: {error}"))?;
        device_sink.log_on_drop(false);
        let player = Player::connect_new(device_sink.mixer());
        if start_paused {
            player.pause();
        }

        let file =
            File::open(path).map_err(|error| format!("Could not open preview audio: {error}"))?;
        let decoder = Decoder::try_from(file)
            .map_err(|error| format!("Could not decode preview audio: {error}"))?;
        player.append(decoder);
        if !start_paused {
            player.play();
        }

        self.device_sink = Some(device_sink);
        self.player = Some(player);
        self.track_id = Some(track_id);
        self.track_path = Some(path.to_path_buf());
        self.last_error = None;
        Ok(())
    }

    fn refresh_finished_sink(&mut self) {
        if self.player.as_ref().is_some_and(Player::empty) {
            self.player = None;
            self.device_sink = None;
        }
    }
}
