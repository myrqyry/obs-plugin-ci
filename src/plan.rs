use serde::Serialize;

#[derive(Serialize)]
pub struct Row {
    target: &'static str,
    label: &'static str,
    runner: &'static str,
    obs: &'static str,
    extension: &'static str,
    emit_source: bool,
}

#[derive(Serialize)]
pub struct Matrix {
    include: Vec<Row>,
}

fn valid_identifier(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && bytes.all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}

pub fn plan(
    module: &str,
    package_name: &str,
    minimum_tests: &str,
    profile: &str,
    driver: &str,
    source_package: bool,
    selection: (&[String], &[String]),
) -> Result<Matrix, String> {
    for (key, name) in [
        ("MODULE", module),
        (
            "PUBLIC_NAME",
            if package_name.is_empty() {
                module
            } else {
                package_name
            },
        ),
    ] {
        if !valid_identifier(name) {
            return Err(format!(
                "Invalid {key}: expected an alphanumeric identifier with hyphens or underscores"
            ));
        }
    }
    if !minimum_tests
        .as_bytes()
        .first()
        .is_some_and(|c| (b'1'..=b'9').contains(c))
        || !minimum_tests.bytes().all(|c| c.is_ascii_digit())
    {
        return Err("Invalid MINIMUM_TESTS: expected a positive integer".into());
    }
    if !matches!(profile, "full" | "linux-obs32") || !matches!(driver, "native" | "caller") {
        return Err("Unsupported matrix profile or driver".into());
    }
    if source_package && driver != "caller" {
        return Err("Source packages require a caller-owned verifier".into());
    }
    let (targets, versions) = selection;
    for (kind, values, allowed) in [
        (
            "target",
            targets,
            &["linux-x86_64", "windows-x64", "macos-arm64", "macos-x86_64"][..],
        ),
        ("OBS version", versions, &["32.2.0", "33.0.3"][..]),
    ] {
        let mut seen = std::collections::BTreeSet::new();
        for value in values {
            if !allowed.contains(&value.as_str()) || !seen.insert(value) {
                return Err(format!("Invalid or duplicate {kind}: {value}"));
            }
        }
    }
    let mut include = Vec::new();
    for (obs, linux_runner) in [("32.2.0", "ubuntu-24.04"), ("33.0.3", "ubuntu-26.04")] {
        for (target, label, runner, extension) in [
            ("linux-x86_64", "Linux x86_64", linux_runner, "tar.gz"),
            ("windows-x64", "Windows x64", "windows-2025-vs2026", "zip"),
            ("macos-arm64", "macOS Apple Silicon", "macos-26", "zip"),
            ("macos-x86_64", "macOS Intel", "macos-26-intel", "zip"),
        ] {
            if profile == "linux-obs32" && (target != "linux-x86_64" || obs != "32.2.0") {
                continue;
            }
            if (!targets.is_empty() && !targets.iter().any(|v| v == target))
                || (!versions.is_empty() && !versions.iter().any(|v| v == obs))
            {
                continue;
            }
            include.push(Row {
                target,
                label,
                runner,
                obs,
                extension,
                emit_source: false,
            });
        }
    }
    if include.is_empty() {
        return Err("Matrix selection is empty".into());
    }
    if source_package {
        let host = include
            .iter_mut()
            .find(|r| r.target == "linux-x86_64")
            .ok_or("Source packages require a Linux matrix target")?;
        host.emit_source = true;
    }
    Ok(Matrix { include })
}
