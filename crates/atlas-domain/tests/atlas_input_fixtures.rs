use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use atlas_domain::{AtlasInputEnvelope, AtlasInputMode};

fn target_fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/external-input/target")
}

fn json_fixture_paths() -> Vec<PathBuf> {
    let mut paths = fs::read_dir(target_fixture_dir())
        .expect("target fixture dir should be readable")
        .map(|entry| entry.expect("fixture dir entry should be readable").path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn load_input(path: &Path) -> AtlasInputEnvelope {
    let raw = fs::read_to_string(path).unwrap_or_else(|err| {
        panic!("failed to read {}: {err}", path.display());
    });
    serde_json::from_str(&raw).unwrap_or_else(|err| {
        panic!(
            "failed to parse {} as AtlasInputEnvelope: {err}",
            path.display()
        );
    })
}

#[test]
fn target_atlas_input_fixtures_parse_and_validate() {
    let paths = json_fixture_paths();
    assert!(
        paths.len() >= 13,
        "expected target fixtures to cover all schema families, got {}",
        paths.len()
    );

    let mut families = BTreeSet::new();
    let mut modes = BTreeSet::new();

    for path in paths {
        let input = load_input(&path);
        input.validate().unwrap_or_else(|err| {
            panic!("{} failed target input validation: {err}", path.display());
        });

        let family = input
            .schema_family()
            .unwrap_or_else(|| panic!("{} has no schema family", path.display()));
        families.insert(family.to_string());
        modes.insert(input.collect.mode.as_str().to_string());

        assert!(
            input.schema.starts_with("atlas.in."),
            "{} must use atlas.in.* schema",
            path.display()
        );
        assert!(
            !input.idempotency_key().trim().is_empty(),
            "{} must produce a non-empty idempotency key",
            path.display()
        );
        assert!(
            !serde_json::to_string(&input.payload)
                .expect("payload should serialize")
                .contains("warp_insight"),
            "{} must not embed producer-native warp_insight payload",
            path.display()
        );
    }

    assert_eq!(
        families,
        BTreeSet::from([
            "artifact".to_string(),
            "bug".to_string(),
            "cmdb".to_string(),
            "correction".to_string(),
            "edge".to_string(),
            "iam".to_string(),
            "k8s".to_string(),
            "manual".to_string(),
            "oncall".to_string(),
            "security".to_string(),
            "sw".to_string(),
            "telemetry".to_string(),
            "vuln".to_string(),
        ])
    );
    assert_eq!(
        modes,
        BTreeSet::from([
            "correction".to_string(),
            "incremental".to_string(),
            "snapshot".to_string(),
            "window".to_string(),
        ])
    );
}

#[test]
fn target_atlas_input_window_fixtures_define_payload_window() {
    for path in json_fixture_paths() {
        let input = load_input(&path);
        if input.collect.mode != AtlasInputMode::Window {
            continue;
        }

        let window = input
            .payload
            .get("window")
            .and_then(serde_json::Value::as_object)
            .unwrap_or_else(|| panic!("{} must define payload.window", path.display()));
        assert!(
            window
                .get("start")
                .and_then(serde_json::Value::as_str)
                .is_some(),
            "{} must define payload.window.start",
            path.display()
        );
        assert!(
            window
                .get("end")
                .and_then(serde_json::Value::as_str)
                .is_some(),
            "{} must define payload.window.end",
            path.display()
        );
    }
}

#[test]
fn target_atlas_input_fixtures_do_not_use_current_contract_fields() {
    let forbidden = [
        "\"source\":{\"kind\"",
        "\"kind\":\"edge\"",
        "\"tenant_ref\"",
        "\"env_ref\"",
        "\"fetched_at\"",
        "\"win_start\"",
        "\"win_end\"",
        "\"net_ifaces\"",
    ];

    for path in json_fixture_paths() {
        let raw = fs::read_to_string(&path).unwrap_or_else(|err| {
            panic!("failed to read {}: {err}", path.display());
        });
        let compact = raw.split_whitespace().collect::<String>();
        for field in forbidden {
            assert!(
                !compact.contains(field),
                "{} contains forbidden current-contract field pattern {field}",
                path.display()
            );
        }
    }
}
