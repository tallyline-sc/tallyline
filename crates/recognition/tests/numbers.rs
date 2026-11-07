// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Shared numeric parsing through the recognition public interface.

#![expect(
    non_snake_case,
    reason = "Requirement tags contain a double underscore."
)]

use proptest::test_runner::{Config, RngAlgorithm, RngSeed, TestError, TestRng, TestRunner};

use tallyline_recognition::api as recognition_api;
use tallyline_recognition::api::model::{DecimalPrices, NumericReading, RawField};
use tallyline_shared_kernel::api::model::{PricePerScu, ScuQuantity};

#[test]
fn r_vlm_5__named_example_keeps_price_and_quantity_distinct()
-> Result<(), Box<dyn std::error::Error>> {
    assert_eq!(
        recognition_api::parse_quantity_text(&RawField::Read("333".to_owned())),
        NumericReading::Candidates {
            raw: Some(ScuQuantity::whole(333)),
            substituted: None,
            confusable_positions: vec![]
        }
    );
    assert_eq!(
        recognition_api::parse_price_text(
            &RawField::Read("3,237".to_owned()),
            DecimalPrices::WholeOnly
        ),
        NumericReading::Candidates {
            raw: Some("3237".parse::<PricePerScu>()?),
            substituted: None,
            confusable_positions: vec![]
        }
    );
    Ok(())
}

#[test]
fn r_vlm_5__numeric_reading_never_silently_repairs_or_rounds()
-> Result<(), Box<dyn std::error::Error>> {
    let read = |text: &str| RawField::Read(text.to_owned());
    assert_eq!(
        recognition_api::parse_price_text(&read("1.5K"), DecimalPrices::WholeOnly),
        NumericReading::Candidates {
            raw: Some("1500".parse::<PricePerScu>()?),
            substituted: None,
            confusable_positions: vec![]
        }
    );
    assert_eq!(
        recognition_api::parse_price_text(&read("1,50"), DecimalPrices::WholeOnly),
        NumericReading::Inconsistent
    );
    assert_eq!(
        recognition_api::parse_price_text(&read("1,50"), DecimalPrices::Allowed),
        NumericReading::Candidates {
            raw: Some("1.5".parse::<PricePerScu>()?),
            substituted: None,
            confusable_positions: vec![]
        }
    );
    assert_eq!(
        recognition_api::parse_quantity_text(&read("1.5K")),
        NumericReading::Unreadable
    );
    assert_eq!(
        recognition_api::parse_price_text(&read("B,585"), DecimalPrices::WholeOnly),
        NumericReading::Candidates {
            raw: None,
            substituted: Some("8585".parse::<PricePerScu>()?),
            confusable_positions: vec![0]
        }
    );
    assert_eq!(
        recognition_api::parse_quantity_text(&read("IOQDSZGl|B")),
        NumericReading::Candidates {
            raw: None,
            substituted: Some("1000526118".parse::<ScuQuantity>()?),
            confusable_positions: (0..10).collect()
        }
    );
    for text in [
        "?",
        "12?",
        "",
        " 333",
        "333 ",
        "333 SCU",
        "¤333",
        "333/SCU",
        "1 12,000",
        "3,,237",
        "3,23,7",
        "1e3",
        "-1",
        "+1",
        "NaN",
        "1k",
        "0.00000000000000000000000000001",
        "79228162514264337593543950336",
        "79228162514264337593543950335M",
    ] {
        assert_eq!(
            recognition_api::parse_price_text(&read(text), DecimalPrices::Allowed),
            NumericReading::Unreadable,
            "accepted unsupported number: {text}"
        );
    }
    assert_eq!(
        recognition_api::parse_price_text(&RawField::Unreadable, DecimalPrices::WholeOnly),
        NumericReading::Unreadable
    );
    assert_eq!(
        recognition_api::parse_quantity_text(&read("333K")),
        NumericReading::Candidates {
            raw: Some(ScuQuantity::whole(333000)),
            substituted: None,
            confusable_positions: vec![]
        }
    );
    Ok(())
}

fn numeric_property_runner() -> TestRunner {
    const SEED: [u8; 16] = [0x90, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

    let config = Config {
        cases: 256,
        max_local_rejects: 1024,
        max_global_rejects: 1024,
        max_flat_map_regens: 1024,
        failure_persistence: None,
        source_file: None,
        test_name: None,
        max_shrink_time: 1000,
        max_shrink_iters: 1024,
        max_default_size_range: 100,
        result_cache: proptest::test_runner::noop_result_cache,
        verbose: 0,
        rng_algorithm: RngAlgorithm::XorShift,
        rng_seed: RngSeed::Fixed(144),
        ..Config::default()
    };
    TestRunner::new_with_rng(config, TestRng::from_seed(RngAlgorithm::XorShift, &SEED))
}

#[test]
fn r_vlm_5__whole_scu_roundtrips_without_price_coercion() -> Result<(), TestError<u32>> {
    numeric_property_runner().run(&(0_u32..=u32::MAX), |value| {
        let result = recognition_api::parse_quantity_text(&RawField::Read(value.to_string()));
        proptest::prop_assert_eq!(
            result,
            NumericReading::Candidates {
                raw: Some(ScuQuantity::whole(value)),
                substituted: None,
                confusable_positions: vec![]
            }
        );
        Ok(())
    })
}

#[test]
fn r_vlm_5__thousands_groups_preserve_exact_values() -> Result<(), TestError<(u32, u32)>> {
    numeric_property_runner().run(&(1_u32..=999, 0_u32..=999), |(prefix, tail)| {
        let result =
            recognition_api::parse_quantity_text(&RawField::Read(format!("{prefix},{tail:03}")));
        proptest::prop_assert_eq!(
            result,
            NumericReading::Candidates {
                raw: Some(ScuQuantity::whole(prefix * 1000 + tail)),
                substituted: None,
                confusable_positions: vec![]
            }
        );
        Ok(())
    })
}

#[test]
fn r_vlm_5__unknown_trailing_characters_never_invent_a_number() -> Result<(), TestError<(u32, char)>>
{
    let strategy = (
        0_u32..=u32::MAX,
        proptest::sample::select(vec!['?', 'X', '/', '¤', '-', '+', '\n']),
    );
    numeric_property_runner().run(&strategy, |(value, suffix)| {
        let result =
            recognition_api::parse_quantity_text(&RawField::Read(format!("{value}{suffix}")));
        proptest::prop_assert_eq!(result, NumericReading::Unreadable);
        Ok(())
    })
}

const PROFILE_CHILD: &str = "TALLYLINE_PROPERTY_PROFILE_CHILD";
const PROFILE_NAME: &str = "r_vlm_5__fixed_property_profile_ignores_hostile_environment";

#[test]
fn r_vlm_5__fixed_property_profile_ignores_hostile_environment()
-> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os(PROFILE_CHILD).is_some() {
        return verify_profile_child();
    }
    assert_eq!(
        profile_sample(false)?,
        profile_sample(true)?,
        "ambient configuration changed generated values"
    );
    Ok(())
}

fn verify_profile_child() -> Result<(), Box<dyn std::error::Error>> {
    use std::cell::RefCell;
    let mut runner = numeric_property_runner();
    let config = runner.config();
    assert_eq!(config.cases, 256);
    assert_eq!(config.max_local_rejects, 1024);
    assert_eq!(config.max_global_rejects, 1024);
    assert_eq!(config.max_flat_map_regens, 1024);
    assert!(config.failure_persistence.is_none());
    assert!(config.source_file.is_none());
    assert!(config.test_name.is_none());
    assert_eq!(config.max_shrink_time, 1000);
    assert_eq!(config.max_shrink_iters, 1024);
    assert_eq!(config.max_default_size_range, 100);
    assert_eq!(config.verbose, 0);
    assert_eq!(config.rng_algorithm, RngAlgorithm::XorShift);
    assert_eq!(config.rng_seed, RngSeed::Fixed(144));
    let values = RefCell::new(Vec::new());
    runner.run(&(0_u32..=u32::MAX), |value| {
        let parsed = recognition_api::parse_quantity_text(&RawField::Read(value.to_string()));
        proptest::prop_assert_eq!(
            parsed,
            NumericReading::Candidates {
                raw: Some(ScuQuantity::whole(value)),
                substituted: None,
                confusable_positions: vec![],
            }
        );
        values.borrow_mut().push(value);
        Ok(())
    })?;
    assert_eq!(values.borrow().len(), 256);
    let calls = std::cell::Cell::new(0);
    numeric_property_runner().run(&proptest::strategy::Just(333_u32), |value| {
        proptest::prop_assert_eq!(
            recognition_api::parse_quantity_text(&RawField::Read(value.to_string())),
            NumericReading::Candidates {
                raw: Some(ScuQuantity::whole(333)),
                substituted: None,
                confusable_positions: vec![],
            }
        );
        calls.set(calls.get() + 1);
        Ok(())
    })?;
    assert_eq!(calls.get(), 256, "result caching must not suppress cases");
    r_vlm_5__whole_scu_roundtrips_without_price_coercion()?;
    r_vlm_5__thousands_groups_preserve_exact_values()?;
    r_vlm_5__unknown_trailing_characters_never_invent_a_number()?;
    use std::io::Write;
    writeln!(std::io::stdout(), "PROFILE_VALUES={:?}", values.borrow())?;
    Ok(())
}

fn profile_sample(hostile: bool) -> Result<String, Box<dyn std::error::Error>> {
    use std::process::Command;
    let mut command = Command::new(std::env::current_exe()?);
    command.args(["--exact", PROFILE_NAME, "--nocapture"]);
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("PROPTEST_") {
            command.env_remove(key);
        }
    }
    command.env(PROFILE_CHILD, "1");
    if hostile {
        command.envs([
            ("PROPTEST_CASES", "0"),
            ("PROPTEST_MAX_LOCAL_REJECTS", "0"),
            ("PROPTEST_MAX_GLOBAL_REJECTS", "0"),
            ("PROPTEST_MAX_FLAT_MAP_REGENS", "0"),
            ("PROPTEST_MAX_SHRINK_TIME", "1"),
            ("PROPTEST_MAX_SHRINK_ITERS", "0"),
            ("PROPTEST_MAX_DEFAULT_SIZE_RANGE", "1"),
            ("PROPTEST_VERBOSE", "9"),
            ("PROPTEST_RNG_ALGORITHM", "cc"),
            ("PROPTEST_RNG_SEED", "999"),
            ("PROPTEST_DISABLE_FAILURE_PERSISTENCE", "0"),
        ]);
    }
    let output = command.output()?;
    assert!(
        output.status.success(),
        "child failed: {}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout)?;
    Ok(text
        .lines()
        .find(|line| line.starts_with("PROFILE_VALUES="))
        .ok_or("missing generated values")?
        .to_owned())
}
