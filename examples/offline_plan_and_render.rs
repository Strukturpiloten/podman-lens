//! Build and render a deterministic deployment without contacting or executing Podman.

use std::io;

use podman_lens::{
    ContainerIntent, DeploymentIntent, DeploymentRendering, DeploymentResource, DeploymentResourceId,
    ExternalPrecondition, ImageIntent, ImagePullPolicy, ImageSource, NetworkAttachment, ObservedApiVersion,
    ObservedPodmanVersion, ResourceKind, TargetExecutionContext, TargetProfile, artifact::deployment_v1,
    plan_deployment, render_deployment,
};

fn id(kind: ResourceKind, name: &str) -> podman_lens::PodmanLensResult<DeploymentResourceId> {
    DeploymentResourceId::new(kind, name)
}

pub(crate) fn build_example() -> Result<(podman_lens::DeploymentPlan, DeploymentRendering), Box<dyn std::error::Error>>
{
    let mut target = TargetProfile::new(
        ObservedPodmanVersion::parse("6.1.0")?,
        ObservedApiVersion::parse("6.1.0")?,
    )?;
    target.set_execution_context(TargetExecutionContext::Rootless);

    let network = id(ResourceKind::Network, "existing-network")?;
    let image = id(ResourceKind::Image, "application-image")?;
    let container = id(ResourceKind::Container, "application")?;
    let mut container_intent = ContainerIntent::new(container, image.clone())?;
    container_intent.add_network(NetworkAttachment::new(network.clone())?)?;

    let mut intent = DeploymentIntent::new(target);
    intent.add_resource(DeploymentResource::ExternalPrecondition(ExternalPrecondition::new(
        network,
    )?));
    intent.add_resource(DeploymentResource::Image(ImageIntent::new(
        image,
        ImageSource::new("registry.example.invalid/team/application:1")?,
        ImagePullPolicy::Missing,
    )?));
    intent.add_resource(DeploymentResource::Container(container_intent));

    let plan = plan_deployment(&intent)
        .plan()
        .cloned()
        .ok_or_else(|| io::Error::other("example deployment intent did not produce a complete plan"))?;
    let rendering = render_deployment(&plan)
        .rendering()
        .cloned()
        .ok_or_else(|| io::Error::other("example deployment plan did not render completely"))?;
    Ok((plan, rendering))
}

#[allow(dead_code)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (plan, rendering) = build_example()?;
    println!("semantic operations: {}", plan.operations().len());
    println!("external preconditions: {}", plan.external_preconditions().len());
    for operation in rendering.operations() {
        println!("cli: podman {:?}", operation.cli().argv());
        println!(
            "libpod: {:?} {} {:?}",
            operation.libpod().method(),
            operation.libpod().path_and_query(),
            operation.libpod().body()
        );
    }
    println!(
        "deployment-v1:\n{}",
        serde_json::to_string_pretty(&deployment_v1::deployment(&rendering))?
    );
    println!("review-script:\n{}", rendering.shell_script());
    Ok(())
}
