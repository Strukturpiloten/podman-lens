//! Public documentation and executable-example contracts.

use std::{collections::BTreeSet, fs, path::Path};

use podman_lens::{
    DeploymentIntent, DeploymentResource, DeploymentResourceId, DiagnosticCode, ImageIntent, ImagePullPolicy,
    ImageSource, ObservedApiVersion, ObservedPodmanVersion, OpaqueReference, ResourceKind, SensitiveInputReference,
    TargetProfile, artifact::deployment_v1, plan_deployment, render_deployment,
};

#[path = "../examples/offline_plan_and_render.rs"]
mod offline_plan_and_render;

#[cfg(unix)]
#[path = "../examples/read_only_discovery.rs"]
mod read_only_discovery;

const PUBLIC_GUIDES: &[&str] = &[
    "docs/public/index.md",
    "docs/public/acquisition/index.md",
    "docs/public/discovery/index.md",
    "docs/public/grouping/index.md",
    "docs/public/planning-rendering/index.md",
    "docs/public/diagnostics-privacy/index.md",
    "docs/public/compatibility/index.md",
];
const PUBLIC_PAGE_WORD_LIMIT: usize = 900;
const PUBLIC_PARAGRAPH_WORD_LIMIT: usize = 120;
const PLACEHOLDER_PHRASES: &[&str] = &[
    "coming soon",
    "final guide will",
    "placeholder",
    "this guide will",
    "this page will",
    "this section will",
];

fn collect_public_markdown(directory: &Path, files: &mut BTreeSet<String>) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            return Err(format!("public documentation must not contain symlinks: {}", path.display()).into());
        }
        if file_type.is_dir() {
            collect_public_markdown(&path, files)?;
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("md") {
            files.insert(path.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

fn word_count(text: &str) -> usize {
    text.split(|character: char| {
        !(character.is_alphanumeric() || character == '_' || character == '\'' || character == '-')
    })
    .filter(|word| !word.is_empty())
    .count()
}

fn prose_paragraphs(text: &str) -> Vec<String> {
    let mut paragraphs = Vec::new();
    let mut current = String::new();
    let mut in_fence = false;
    for line in text.lines() {
        if line.as_bytes().starts_with(b"\x60\x60\x60") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if line.trim().is_empty() {
            if !current.is_empty() {
                paragraphs.push(std::mem::take(&mut current));
            }
        } else {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(line);
        }
    }
    if !current.is_empty() {
        paragraphs.push(current);
    }
    paragraphs
}

fn target(version: &str) -> Result<TargetProfile, podman_lens::Diagnostic> {
    TargetProfile::new(
        ObservedPodmanVersion::parse(version)?,
        ObservedApiVersion::parse(version)?,
    )
}

fn image_rendering(version: &str) -> Result<podman_lens::RenderingOutcome, podman_lens::Diagnostic> {
    let image = DeploymentResourceId::new(ResourceKind::Image, "guide-image")?;
    let mut intent = DeploymentIntent::new(target(version)?);
    intent.add_resource(DeploymentResource::Image(ImageIntent::new(
        image,
        ImageSource::new("registry.example.invalid/guide/image:1")?,
        ImagePullPolicy::Missing,
    )?));
    let plan = plan_deployment(&intent)
        .plan()
        .cloned()
        .ok_or_else(|| podman_lens::Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent))?;
    Ok(render_deployment(&plan))
}

#[test]
fn public_guide_inventory_and_navigation_are_complete() -> Result<(), Box<dyn std::error::Error>> {
    let expected = PUBLIC_GUIDES
        .iter()
        .map(|path| (*path).to_owned())
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeSet::new();
    collect_public_markdown(Path::new("docs/public"), &mut actual)?;
    assert_eq!(actual, expected, "public guide inventory drifted");

    for path in PUBLIC_GUIDES {
        assert!(Path::new(path).is_file(), "missing public guide {path}");
        let text = fs::read_to_string(path)?;
        assert!(!text.trim().is_empty(), "empty public guide {path}");
        assert_eq!(
            text.lines().filter(|line| line.starts_with("# ")).count(),
            1,
            "public guide must contain exactly one level-one heading: {path}"
        );
        assert!(
            word_count(&text) <= PUBLIC_PAGE_WORD_LIMIT,
            "public guide exceeds {PUBLIC_PAGE_WORD_LIMIT} words: {path}"
        );
        let folded = text.to_lowercase();
        for phrase in PLACEHOLDER_PHRASES {
            assert!(
                !folded.contains(phrase),
                "public guide contains placeholder phrase {phrase}: {path}"
            );
        }
        for paragraph in prose_paragraphs(&text) {
            let trimmed = paragraph.trim_start();
            if trimmed.starts_with(['#', '-', '|', '<']) {
                continue;
            }
            assert!(
                word_count(trimmed) <= PUBLIC_PARAGRAPH_WORD_LIMIT,
                "public paragraph exceeds {PUBLIC_PARAGRAPH_WORD_LIMIT} words: {path}"
            );
        }
    }
    let index = fs::read_to_string(PUBLIC_GUIDES[0])?;
    for destination in [
        "acquisition/",
        "discovery/",
        "grouping/",
        "planning-rendering/",
        "diagnostics-privacy/",
        "compatibility/",
    ] {
        assert!(index.contains(destination), "public index omits {destination}");
    }
    Ok(())
}

#[cfg(unix)]
#[test]
fn acquisition_example_requires_one_explicit_read_only_socket() -> Result<(), Box<dyn std::error::Error>> {
    let path = "/run/user/1000/podman/podman.sock";
    let transport = read_only_discovery::explicit_transport(path.into())?;
    assert_eq!(transport.connection().socket_path(), Path::new(path));

    let source = fs::read_to_string("examples/read_only_discovery.rs")?;
    for required in [
        "env::args_os().nth(1)",
        "UnixConnection::new",
        "ReadOnlyUnixTransport::new",
        "AcquisitionOptions::redacted",
        "acquire_inventory",
    ] {
        assert!(source.contains(required), "acquisition example omits {required}");
    }
    for prohibited in [
        "env::var(",
        "LibpodMethod::Post",
        "LibpodMethod::Delete",
        "Command::new",
    ] {
        assert!(
            !source.contains(prohibited),
            "acquisition example contains {prohibited}"
        );
    }
    Ok(())
}

#[test]
fn offline_example_covers_semantics_both_renderers_and_inert_artifacts() -> Result<(), Box<dyn std::error::Error>> {
    let (plan, rendering) = offline_plan_and_render::build_example()?;
    assert_eq!(plan.external_preconditions().len(), 1);
    assert_eq!(plan.operations().len(), 3);
    assert_eq!(rendering.operations().len(), 3);
    assert!(
        rendering
            .operations()
            .iter()
            .all(|operation| operation.cli().program() == "podman")
    );
    assert!(
        rendering
            .operations()
            .iter()
            .all(|operation| operation.libpod().path_and_query().starts_with("/v6.1.0/libpod/"))
    );
    let artifact = serde_json::to_string_pretty(&deployment_v1::deployment(&rendering))?;
    assert!(artifact.contains("\"schema_version\": 1"));
    assert!(artifact.contains("\"external_preconditions\""));
    let script = rendering.shell_script();
    assert!(script.contains("# Requires external network: 'existing-network'"));
    assert!(script.contains("podman 'image' 'pull'"));
    assert!(script.contains("podman 'container' 'create'"));
    assert!(script.contains("podman 'container' 'start'"));
    Ok(())
}

#[test]
fn compatibility_guide_tracks_catalogue_and_renderer_gates() -> Result<(), Box<dyn std::error::Error>> {
    let guide = fs::read_to_string("docs/public/compatibility/index.md")?;
    let catalogue: serde_json::Value = serde_json::from_slice(&fs::read("catalogue/v1/podman-capabilities.json")?)?;
    let versions = catalogue["reviewed_lines"]
        .as_array()
        .ok_or("reviewed_lines must be an array")?
        .iter()
        .filter(|line| line["output_supported"].as_bool().unwrap_or(true))
        .map(|line| {
            line["observed_podman_version"]
                .as_str()
                .ok_or("observed_podman_version must be a string")
        })
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        versions,
        ["5.4.0", "5.5.0", "5.6.0", "5.7.0", "5.8.6", "6.0.0", "6.1.0"]
    );
    for version in versions {
        assert!(guide.contains(version), "compatibility guide omits {version}");
    }
    assert!(!image_rendering("5.5.0")?.is_success());
    assert!(image_rendering("5.6.0")?.is_success());
    assert!(guide.contains("GitHub issue #3"));
    assert!(guide.contains("manual-only"));
    Ok(())
}

#[test]
fn grouping_guide_is_bound_to_the_network_boundary_fixture() -> Result<(), Box<dyn std::error::Error>> {
    let manifest: serde_json::Value = serde_json::from_slice(&fs::read("fixtures/corpus/manifest.json")?)?;
    let artifact = manifest["artifacts"]
        .as_array()
        .ok_or("artifacts must be an array")?
        .iter()
        .find(|artifact| artifact["artifact"] == "graph-boundaries-6.1.responses.json")
        .ok_or("network-boundary fixture is missing")?;
    let coverage = artifact["coverage"]
        .as_array()
        .ok_or("coverage must be an array")?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect::<Vec<_>>();
    for claim in ["shared prerequisite", "network name and ID crossing", "no crossing"] {
        assert!(coverage.contains(&claim), "fixture omits {claim}");
    }
    let guide = fs::read_to_string("docs/public/grouping/index.md")?;
    assert!(guide.contains("graph-boundaries-6.1.responses.json"));
    assert!(guide.contains("Shared prerequisite"));
    assert!(guide.contains("Pod membership"));
    Ok(())
}

#[test]
fn diagnostics_and_privacy_claims_keep_sensitive_references_redacted() -> Result<(), Box<dyn std::error::Error>> {
    let guide = fs::read_to_string("docs/public/diagnostics-privacy/index.md")?;
    for code in ["PLN0018", "PLN0023", "PLN0033", "PLN0046"] {
        assert!(guide.contains(code), "diagnostics guide omits {code}");
    }
    let secret = SensitiveInputReference::new("vault/GUIDE-SECRET-CANARY")?;
    let credential = OpaqueReference::new("GUIDE-CREDENTIAL-CANARY")?;
    let formatted = format!("{secret:?} {credential:?}");
    assert!(!formatted.contains("GUIDE-SECRET-CANARY"));
    assert!(!formatted.contains("GUIDE-CREDENTIAL-CANARY"));
    assert!(guide.contains("redacted rather than anonymous"));
    Ok(())
}
