use launchpad_drummer_lib::audio::AudioStatus;

#[test]
fn audio_status_serializes_correctly() {
    let status = AudioStatus {
        state: "running".into(),
        device_name: "Default Speakers".into(),
        sample_rate: 48_000,
        buffer_size: Some(128),
        channels: 2,
        dropouts: 3,
        last_dropout_ago_ms: Some(1500),
    };
    let json = serde_json::to_string(&status).unwrap();
    assert!(json.contains("\"state\":\"running\""));
    assert!(json.contains("\"sample_rate\":48000") || json.contains("\"sampleRate\":48000"));
    assert!(json.contains("\"dropouts\":3"));
    assert!(json.contains("\"lastDropoutAgoMs\":1500"));
}
