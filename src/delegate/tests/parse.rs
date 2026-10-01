// T109 family: parse — resume/max_tokens payload parsing (T39).
// Moved bytes byte-identical (T84 rule) from delegate.rs's test
// module; every test here lives in exactly one family file.
// T109 req 4 count-pin anchor (see mod.rs's pin): this family's
// #[test] fn count — a dropped `mod parse;` line fails the pin's
// reference to this const to compile.
pub(super) const TEST_COUNT: usize = 3;
    use super::*; // the shared harness (delegate::tests) + delegate's own imports

    /// T58: the parse — absent → `false`; `false` → `false`; `true` → `true`;
    /// a non-boolean (string "true", number 1) is a tool error naming the
    /// boolean requirement, never a silent fresh start (a caller that asked to
    /// resume and silently got a fresh child would lose the prior run).
    #[test]
    fn delegate_resume_parse_absent_false_true_and_rejects_non_boolean() {
        assert!(!delegate_resume(&json!({})).unwrap());
        assert!(!delegate_resume(&json!({"resume": false})).unwrap());
        assert!(delegate_resume(&json!({"resume": true})).unwrap());
        for bad in [json!("true"), json!(1), json!(0)] {
            let err = delegate_resume(&json!({"resume": bad}))
                .unwrap_err()
                .to_string();
            assert!(err.contains("boolean"), "{bad} → {err}");
        }
    }

    /// T39: the parse — absent → `None`; valid → `Some`; `< 1` (0, negative)
    /// → error naming the constraint; non-integer (string, fractional) →
    /// error naming the integer requirement. Zero never parses into `Some(0)`
    /// (which the child CLI would read as "unlimited").
    #[test]
    fn delegate_max_tokens_parse_absent_valid_and_rejects() {
        assert_eq!(delegate_max_tokens(&json!({})).unwrap(), None);
        assert_eq!(
            delegate_max_tokens(&json!({"max_tokens": 250_000})).unwrap(),
            Some(250_000)
        );
        assert_eq!(delegate_max_tokens(&json!({"max_tokens": 1})).unwrap(), Some(1));
        for bad in [json!(0), json!(-5)] {
            let err = delegate_max_tokens(&json!({"max_tokens": bad}))
                .unwrap_err()
                .to_string();
            assert!(err.contains("at least 1"), "{bad} → {err}");
        }
        for bad in [json!("250000"), json!(250000.5)] {
            let err = delegate_max_tokens(&json!({"max_tokens": bad}))
                .unwrap_err()
                .to_string();
            assert!(err.contains("integer"), "{bad} → {err}");
        }
    }

    /// T183: the `env` parse — absent (or null) → an EMPTY map (byte-identical
    /// spawn); a well-formed map parses to its entries (in the JSON object's
    /// sorted key order — serde_json's map is a BTreeMap, so application is
    /// deterministic); the fail-closed legs are tool errors naming the
    /// offending key or the constraint: non-object, non-string value, bad
    /// key (PATH / lowercase / empty / HOME), a 17th entry, a 4 KiB+1 value,
    /// a NUL byte in a value. The 4096-byte boundary value itself is
    /// accepted.
    #[test]
    fn delegate_env_parse_absent_valid_and_rejects() {
        let empty: Vec<(String, String)> = Vec::new();
        assert_eq!(delegate_env_map(&json!({})).unwrap(), empty, "absent");
        assert_eq!(
            delegate_env_map(&json!({"env": null})).unwrap(),
            empty,
            "explicit null is the absent shape (the resume precedent)"
        );
        assert_eq!(
            delegate_env_map(&json!({"env": {"CARGO_TARGET_DIR": "/shared", "CHUG_X_2": "v"}}))
                .unwrap(),
            vec![
                ("CARGO_TARGET_DIR".to_string(), "/shared".to_string()),
                ("CHUG_X_2".to_string(), "v".to_string()),
            ],
            "well-formed map parses, entries in sorted key order (serde_json's map)"
        );
        // Every allowlisted prefix accepts, digits/underscores included, and
        // the regex's zero-length remainder matches too (bare `CARGO_`,
        // bare `RUST`).
        for key in ["CARGO_TARGET_DIR", "CHUG_1_A", "RUSTFLAGS", "RUST", "CARGO_"] {
            assert!(
                delegate_env_map(&json!({"env": {(key): "v"}})).is_ok(),
                "{key} must be allowlisted"
            );
        }
        // Non-object and non-string-value legs.
        let err = delegate_env_map(&json!({"env": "CARGO_X=1"}))
            .unwrap_err()
            .to_string();
        assert!(err.contains("must be an object"), "{err}");
        let err = delegate_env_map(&json!({"env": {"CARGO_X": 3}}))
            .unwrap_err()
            .to_string();
        assert!(err.contains("CARGO_X") && err.contains("must be a string"), "{err}");
        // Bad keys: the error NAMES the offending key and the allowlist.
        for bad in ["PATH", "cargo_target_dir", "", "HOME", "DYLD_LIBRARY_PATH"] {
            let err = delegate_env_map(&json!({"env": {(bad): "v"}}))
                .unwrap_err()
                .to_string();
            assert!(err.contains(bad), "{bad} must be named: {err}");
            assert!(
                err.contains("^(CARGO_|CHUG_|RUST)[A-Z0-9_]*$"),
                "{bad}: the error must carry the allowlist regex: {err}"
            );
        }
        // 17 entries → error naming the cap (16 is accepted).
        let mut big = serde_json::Map::new();
        for i in 0..17 {
            big.insert(format!("CARGO_K{i}"), json!("v"));
        }
        let err = delegate_env_map(&json!({"env": big}))
            .unwrap_err()
            .to_string();
        assert!(err.contains("at most 16 entries"), "{err}");
        let mut ok = serde_json::Map::new();
        for i in 0..16 {
            ok.insert(format!("CARGO_K{i}"), json!("v"));
        }
        assert!(delegate_env_map(&json!({"env": ok})).is_ok(), "16 entries fit");
        // 4 KiB + 1 → error; exactly 4 KiB → accepted.
        let oversized = "x".repeat(4097);
        let err = delegate_env_map(&json!({"env": {"CARGO_X": oversized}}))
            .unwrap_err()
            .to_string();
        assert!(err.contains("4 KiB"), "{err}");
        let boundary = "x".repeat(4096);
        assert!(delegate_env_map(&json!({"env": {"CARGO_X": boundary}})).is_ok());
        // NUL in a value → error naming the key.
        let err = delegate_env_map(&json!({"env": {"CARGO_X": "a\u{0000}b"}}))
            .unwrap_err()
            .to_string();
        assert!(err.contains("CARGO_X") && err.contains("NUL"), "{err}");
    }

