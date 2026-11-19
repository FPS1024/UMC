use std::{
    io::{self, BufRead, Write},
    path::PathBuf,
};
use umc_api::RpcRequest;
use umc_core::UmcCore;

async fn wait_for_task(
    core: &UmcCore,
    handle: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let Some(task_id) = handle.get("task_id").and_then(serde_json::Value::as_str) else {
        return Ok(handle);
    };
    eprintln!("Task {task_id} started");
    let mut sequence = 0u64;
    loop {
        let events = core.dispatch(RpcRequest {
            jsonrpc: "2.0".into(), id: Some(serde_json::json!(8)), method: "task.events".into(),
            params: serde_json::json!({"task_id":task_id,"after_sequence":sequence,"wait_ms":750}),
        }).await;
        if let Some(error) = events.error {
            return Err(error.message);
        }
        for event in events
            .result
            .unwrap_or_default()
            .as_array()
            .into_iter()
            .flatten()
        {
            sequence = sequence.max(event["sequence"].as_u64().unwrap_or(sequence));
            match event["event"].as_str().unwrap_or("") {
                "install.progress" => eprintln!(
                    "{}",
                    event["params"]["stage"]
                        .as_str()
                        .or(event["data"]["stage"].as_str())
                        .unwrap_or("installing")
                ),
                "download.progress" => {
                    if let Some(percent) = event["data"]["percentage"].as_f64() {
                        eprintln!("download {:.0}%", percent);
                    }
                }
                "download.retry" => eprintln!(
                    "retrying a download: {}",
                    event["data"]["error"].as_str().unwrap_or("network error")
                ),
                _ => {}
            }
        }
        let status = core
            .dispatch(RpcRequest {
                jsonrpc: "2.0".into(),
                id: Some(serde_json::json!(9)),
                method: "task.get".into(),
                params: serde_json::json!({"task_id":task_id}),
            })
            .await;
        if let Some(error) = status.error {
            return Err(error.message);
        }
        let info = status.result.unwrap_or_default();
        match info["status"].as_str() {
            Some("succeeded") => {
                return Ok(serde_json::json!({"task_id":task_id,"result":info["result"]}));
            }
            Some("failed" | "cancelled" | "interrupted") => {
                return Err(info["error"].as_str().unwrap_or("Task failed").to_owned());
            }
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".into());
    let data_dir = std::env::var_os("UMC_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("umc-data"));
    match command.as_str() {
        "rpc" if args.next().as_deref() == Some("--stdio") => {
            let core = UmcCore::new(data_dir);
            for line in io::stdin().lock().lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                let response = match serde_json::from_str::<RpcRequest>(&line) {
                    Ok(request) if request.id.is_none() => continue,
                    Ok(request) => core.dispatch(request).await,
                    Err(error) => umc_api::RpcResponse {
                        jsonrpc: "2.0".into(),
                        id: None,
                        result: None,
                        error: Some(umc_api::RpcError {
                            code: -32700,
                            message: error.to_string(),
                            data: None,
                        }),
                    },
                };
                serde_json::to_writer(&mut io::stdout().lock(), &response)?;
                writeln!(io::stdout().lock())?;
                io::stdout().flush()?;
            }
        }
        "version" if args.next().as_deref() == Some("list") => {
            let core = UmcCore::new(data_dir);
            let response = core
                .dispatch(RpcRequest {
                    jsonrpc: "2.0".into(),
                    id: Some(serde_json::json!(1)),
                    method: "version.list".into(),
                    params: serde_json::Value::Null,
                })
                .await;
            if let Some(err) = response.error {
                return Err(err.message.into());
            }
            for item in response
                .result
                .unwrap_or_default()
                .as_array()
                .into_iter()
                .flatten()
            {
                println!("{}", item["id"].as_str().unwrap_or("?"));
            }
        }
        "instance" => {
            let subcommand = args.next().unwrap_or_default();
            let params = match subcommand.as_str() {
                "list" => serde_json::json!({}),
                "create" => {
                    let name = args
                        .next()
                        .ok_or("usage: umc instance create <name> <minecraft-version>")?;
                    let version = args
                        .next()
                        .ok_or("usage: umc instance create <name> <minecraft-version>")?;
                    serde_json::json!({"name":name,"minecraft_version":version})
                }
                "install" | "remove" | "get" | "repair" => {
                    let instance_id = args
                        .next()
                        .ok_or("usage: umc instance <install|remove|get|repair> <instance-id>")?;
                    serde_json::json!({"instance_id":instance_id})
                }
                _ => {
                    return Err(
                        "usage: umc instance <list|create|install|get|remove|repair> [arguments]"
                            .into(),
                    );
                }
            };
            let method = match subcommand.as_str() {
                "list" => "instance.list",
                "create" => "instance.create",
                "install" => "instance.install",
                "get" => "instance.get",
                "repair" => "instance.repair",
                _ => "instance.remove",
            };
            let core = UmcCore::new(data_dir);
            let response = core
                .dispatch(RpcRequest {
                    jsonrpc: "2.0".into(),
                    id: Some(serde_json::json!(2)),
                    method: method.into(),
                    params,
                })
                .await;
            if let Some(err) = response.error {
                return Err(err.message.into());
            }
            let result = response.result.unwrap_or_default();
            let result = if subcommand == "install" {
                wait_for_task(&core, result)
                    .await
                    .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?
            } else {
                result
            };
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        "java" => {
            let subcommand = args.next().unwrap_or_default();
            let (method, params) = match subcommand.as_str() {
                "detect" => {
                    let minimum = args.next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(8);
                    ("java.detect", serde_json::json!({"minimum_major":minimum}))
                }
                "runtimes" => ("java.runtime.list", serde_json::Value::Null),
                "install" => {
                    let version = args.next().ok_or("usage: umc java install <minecraft-version>")?;
                    ("java.runtime.install", serde_json::json!({"minecraft_version":version}))
                }
                "remove" => {
                    let runtime_id = args.next().ok_or("usage: umc java remove <runtime-id>")?;
                    ("java.runtime.remove", serde_json::json!({"runtime_id":runtime_id}))
                }
                _ => return Err("usage: umc java <detect [minimum-major]|runtimes|install <minecraft-version>|remove <runtime-id>>".into()),
            };
            let core = UmcCore::new(data_dir);
            let response = core
                .dispatch(RpcRequest {
                    jsonrpc: "2.0".into(),
                    id: Some(serde_json::json!(5)),
                    method: method.into(),
                    params,
                })
                .await;
            if let Some(err) = response.error {
                return Err(err.message.into());
            }
            let result = response.result.unwrap_or_default();
            let result = if subcommand == "install" {
                wait_for_task(&core, result)
                    .await
                    .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?
            } else {
                result
            };
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        "process" => {
            let subcommand = args.next().unwrap_or_default();
            let (method, params) = match subcommand.as_str() {
                "list" => ("process.list", serde_json::Value::Null),
                "status" | "stop" => {
                    let process_id = args
                        .next()
                        .ok_or("usage: umc process <status|stop> <process-id>")?;
                    (
                        if subcommand == "status" {
                            "process.status"
                        } else {
                            "process.stop"
                        },
                        serde_json::json!({"process_id":process_id}),
                    )
                }
                _ => {
                    return Err(
                        "usage: umc process <list|status <process-id>|stop <process-id>>".into(),
                    );
                }
            };
            let response = UmcCore::new(data_dir)
                .dispatch(RpcRequest {
                    jsonrpc: "2.0".into(),
                    id: Some(serde_json::json!(6)),
                    method: method.into(),
                    params,
                })
                .await;
            if let Some(err) = response.error {
                return Err(err.message.into());
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&response.result.unwrap_or_default())?
            );
        }
        "task" => {
            let subcommand = args.next().unwrap_or_default();
            let task_id = args
                .next()
                .ok_or("usage: umc task <get|cancel|events> <task-id>")?;
            let launcher = umc_sdk::Launcher::builder()
                .data_dir(data_dir)
                .connect_daemon()
                .build()
                .await?;
            let result = match subcommand.as_str() {
                "get" => serde_json::to_value(launcher.task(&task_id).await?)?,
                "cancel" => {
                    launcher.cancel_task(&task_id).await?;
                    serde_json::json!({"task_id":task_id,"status":"cancel_requested"})
                }
                "events" => {
                    let after_sequence =
                        args.next().and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
                    serde_json::to_value(
                        launcher.task_events(&task_id, after_sequence, 1000).await?,
                    )?
                }
                _ => return Err("usage: umc task <get|cancel|events> <task-id>".into()),
            };
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        "launch" => {
            let instance_id = args
                .next()
                .ok_or("usage: umc launch <instance-id> <offline-username> --offline")?;
            let username = args
                .next()
                .ok_or("usage: umc launch <instance-id> <offline-username> --offline")?;
            if args.next().as_deref() != Some("--offline") {
                return Err("Offline launch must be explicitly enabled with --offline".into());
            }
            let response = UmcCore::new(data_dir.clone()).dispatch(RpcRequest { jsonrpc: "2.0".into(), id: Some(serde_json::json!(3)), method: "launch.start".into(), params: serde_json::json!({"instance_id":instance_id,"username":username,"offline_mode":true}) }).await;
            if let Some(err) = response.error {
                return Err(err.message.into());
            }
            let process = response.result.unwrap_or_default();
            println!("{}", serde_json::to_string_pretty(&process)?);
            let process_id = process["process_id"]
                .as_str()
                .ok_or("Core returned no process_id")?
                .to_owned();
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let status = UmcCore::new(data_dir.clone())
                    .dispatch(RpcRequest {
                        jsonrpc: "2.0".into(),
                        id: Some(serde_json::json!(4)),
                        method: "process.status".into(),
                        params: serde_json::json!({"process_id":process_id}),
                    })
                    .await;
                if let Some(err) = status.error {
                    return Err(err.message.into());
                }
                let state = status.result.unwrap_or_default();
                if matches!(state["status"].as_str(), Some("exited" | "failed")) {
                    println!("{}", serde_json::to_string_pretty(&state)?);
                    break;
                }
            }
        }
        "auth" => {
            let subcommand = args.next().unwrap_or_default();
            if subcommand != "login" {
                return Err("usage: umc auth login [--no-browser] [--redirect-uri <uri>]".into());
            }
            let mut open_browser = true;
            let mut redirect_uri = None;
            while let Some(argument) = args.next() {
                match argument.as_str() {
                    "--no-browser" => open_browser = false,
                    "--redirect-uri" => {
                        redirect_uri = Some(args.next().ok_or("--redirect-uri requires a value")?);
                    }
                    other => return Err(format!("Unknown auth option: {other}").into()),
                }
            }

            let mut config = umc_core::auth::MicrosoftOAuthConfig::default();
            if let Some(uri) = redirect_uri {
                config.redirect_uri = uri;
            }

            let flow = umc_core::auth::LoginFlow::start_with_config(config, open_browser)
                .await
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            eprintln!("Redirect URI: {}", flow.redirect_uri());
            eprintln!("Microsoft sign-in URL:\n{}\n", flow.auth_url());
            if let Some(error) = flow.browser_error() {
                eprintln!("Could not open the browser automatically: {error}");
                eprintln!("Open the URL above manually to continue.");
            }
            eprintln!("Waiting for the Microsoft sign-in callback...");

            let oauth = flow
                .wait()
                .await
                .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;
            println!("Microsoft OAuth complete.");

            let session = umc_core::auth::complete_minecraft_login_with_progress(&oauth, |stage| {
                eprintln!("{stage}...")
            })
            .await
            .map_err(|e| -> Box<dyn std::error::Error> { e.into() })?;

            println!("Minecraft login complete.");
            println!("Account: {} ({})", session.profile.name, session.profile.id);
            println!(
                "Minecraft token expires in: {} seconds",
                session.minecraft_expires_in.unwrap_or(0)
            );
            println!(
                "Microsoft refresh token: {}",
                if session.microsoft_refresh_token.is_some() {
                    "received"
                } else {
                    "not returned"
                }
            );
            println!("Tokens are not persisted by this step.");
        }
        _ => println!(
            "UMC — Universal Minecraft Core\n\nCommands:\n  umc version list\n  umc instance create <name> <version>\n  umc instance list|get|install|remove|repair <instance-id>\n  umc java detect [minimum-major]\n  umc java runtimes|install|remove ...\n  umc task get|cancel|events <task-id>\n  umc process list|status|stop ...\n  umc launch <instance-id> <username> --offline\n  umc auth login [--no-browser] [--redirect-uri <uri>]\n  umc rpc --stdio\n  umcd  (start local IPC daemon)\n\nSet UMC_DATA_DIR to choose the data directory."
        ),
    }
    Ok(())
}
