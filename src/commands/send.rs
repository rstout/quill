use crate::commands::request_status_submit;
use crate::lib::{
    environment::Environment,
    read_json,
    sign::signed_message::{Ingress, IngressWithRequestId},
    DfxResult,
};
use anyhow::anyhow;
use clap::Clap;
use ic_agent::agent::ReplicaV2Transport;
use ic_agent::{agent::http_transport::ReqwestHttpReplicaV2Transport, RequestId};
use std::str::FromStr;

/// Send a signed message
#[derive(Clap)]
pub struct SendOpts {
    /// Path to the signed message
    file_name: String,

    /// Will display the signed message, but not send it.
    #[clap(long)]
    dry_run: bool,
}

pub async fn exec(env: &dyn Environment, opts: SendOpts) -> DfxResult {
    let json = read_json(opts.file_name)?;
    if let Ok(val) = serde_json::from_str::<IngressWithRequestId>(&json) {
        send(env, val.ingress, opts.dry_run).await?;
        request_status_submit::submit(env, val.request_status).await?;
    } else {
        return Err(anyhow!("Invalid JSON content"));
    }
    Ok(())
}

async fn send(env: &dyn Environment, message: Ingress, dry_run: bool) -> DfxResult {
    let (sender, canister_id, method_name, args) = message.parse()?;

    println!("  Call type:   {}", message.call_type);
    println!("  Sender:      {}", sender);
    println!("  Canister id: {}", canister_id);
    println!("  Method name: {}", method_name);
    println!("  Arguments:   {}", args);

    if dry_run {
        return Ok(());
    }

    // Not using dialoguer because it doesn't support non terminal env like bats e2e
    println!("\nDo you want to send this message? [y/N]");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    if !["y", "yes"].contains(&input.to_lowercase().trim()) {
        return Ok(());
    }

    let network = env
        .get_network_descriptor()
        .providers
        .first()
        .expect("Cannot get network provider (url).")
        .to_string();
    let transport = ReqwestHttpReplicaV2Transport::create(network)?;
    let content = hex::decode(&message.content)?;

    match message.call_type.as_str() {
        "query" => {
            let response = transport.query(canister_id, content).await?;
            print!("Response: ");
            println!("{}", hex::encode(response));
        }
        "update" => {
            let request_id = RequestId::from_str(
                &message
                    .request_id
                    .expect("Cannot get request_id from the update message"),
            )?;
            transport.call(canister_id, content, request_id).await?;
            let request_id = format!("0x{}", String::from(request_id));
            println!("Request ID: {}", request_id);
        }
        // message.validate() guarantee that call_type must be query or update
        _ => unreachable!(),
    }
    Ok(())
}
