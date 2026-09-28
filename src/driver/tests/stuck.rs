// T104 family: stuck — is_stuck tripwire unit pins. Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
    use super::*; // the shared harness (driver::tests) + driver's own imports
    #[test]
    fn tripwire_fires_on_three_identical_errors() {
        let recent: Vec<ToolResult> = (0..3)
            .map(|_| ToolResult {
                content: "boom".to_string(),
                is_error: true,
                images: Vec::new(),
            })
            .collect();
        assert!(is_stuck(&recent));
    }

    #[test]
    fn tripwire_ignores_different_errors() {
        let recent = vec![
            ToolResult {
                content: "boom".to_string(),
                is_error: true,
                images: Vec::new(),
            },
            ToolResult {
                content: "boom".to_string(),
                is_error: true,
                images: Vec::new(),
            },
            ToolResult {
                content: "different failure".to_string(),
                is_error: true,
                images: Vec::new(),
            },
        ];
        assert!(!is_stuck(&recent));
    }

    #[test]
    fn tripwire_requires_all_errors() {
        let recent = vec![
            ToolResult {
                content: "boom".to_string(),
                is_error: true,
                images: Vec::new(),
            },
            ToolResult {
                content: "boom".to_string(),
                is_error: true,
                images: Vec::new(),
            },
            ToolResult {
                content: "boom".to_string(),
                is_error: false,
                images: Vec::new(),
            },
        ];
        assert!(!is_stuck(&recent));
    }

    #[test]
    fn tripwire_needs_full_window() {
        let recent: Vec<ToolResult> = (0..2)
            .map(|_| ToolResult {
                content: "boom".to_string(),
                is_error: true,
                images: Vec::new(),
            })
            .collect();
        assert!(!is_stuck(&recent));
    }

    #[test]
    fn tripwire_compares_first_500_chars() {
        let long_err = format!("{}{}", "e".repeat(600), "A");
        let long_err2 = format!("{}{}", "e".repeat(600), "B");
        let mut recent = Vec::new();
        for content in [long_err.clone(), long_err, long_err2] {
            recent.push(ToolResult {
                content,
                is_error: true,
                images: Vec::new(),
            });
        }
        // identical in the first 500 chars despite differing tails
        assert!(is_stuck(&recent));
    }

