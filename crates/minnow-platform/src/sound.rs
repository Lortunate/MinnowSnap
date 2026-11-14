use minnow_application::ports::{ErrorKind, OperationError, OperationResult};

/// Blocking native playback. The host decides whether sound is enabled and
/// schedules this finite operation on its executor.
pub fn play_shutter(sound_bytes: &'static [u8]) -> OperationResult<()> {
    let device = rodio::DeviceSinkBuilder::open_default_sink()
        .map_err(|error| OperationError::new(ErrorKind::Native, format!("failed to open audio device: {error}")))?;
    let player = rodio::Player::connect_new(device.mixer());
    let source = rodio::Decoder::new(std::io::Cursor::new(sound_bytes))
        .map_err(|error| OperationError::new(ErrorKind::Validation, format!("failed to decode shutter sound: {error}")))?;
    player.append(source);
    player.sleep_until_end();
    Ok(())
}
