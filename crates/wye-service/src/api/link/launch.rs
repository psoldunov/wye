//! Starting the target (PIPE-15, LAUNCH-01 to LAUNCH-07) and the
//! notifications around it (PIPE-02, LAUNCH-07).

use wye_api::Error;
use wye_core::Rejected;
use wye_desktop::launch::ACTIVATION_ENV;

use super::incoming::Activation;
use super::route::{Alternative, Plan};
use super::{FailedLaunch, Result};
use crate::context::ServiceContext;
use crate::platform::{LaunchCommand, Notification};

/// Prefix of the notification action that opens an alternative.
pub(crate) const OPEN_ACTION: &str = "open:";

/// Start `plan`'s command with `activation` (LAUNCH-03), then move it into a
/// scope of its own (LAUNCH-06; failure is logged, never fatal). A failure
/// to start is reported with a notification offering other targets
/// (LAUNCH-07) and returned as `Failed`.
pub(crate) async fn open(ctx: &ServiceContext, plan: Plan, activation: &Activation) -> Result<()> {
    let command = match &plan.command {
        Ok(command) => platform_command(&plan, command, activation),
        Err(error) => return failed(ctx, plan.clone(), error.to_string(), activation).await,
    };
    let platform = ctx.platform();
    match platform.launcher.launch(&command).await {
        Ok(pid) => {
            if let Err(error) = platform.scope.adopt(pid, &command).await {
                tracing::info!(%error, pid, "launched app stays in Wye's own unit");
            }
            tracing::info!(target = %plan.target, url = %plan.url, pid, "opened");
            // PIPE-16: only a link that opened is recorded.
            if let Some(entry) = plan.history {
                crate::api::history::record_entry(ctx, entry).await;
            }
            Ok(())
        }
        Err(error) => failed(ctx, plan, error.to_string(), activation).await,
    }
}

/// The command as the launcher takes it: the activation token set, or
/// removed for a background launch (LAUNCH-03, LAUNCH-04).
fn platform_command(
    plan: &Plan,
    command: &wye_desktop::LaunchCommand,
    activation: &Activation,
) -> LaunchCommand {
    let [token_name, startup_name] = ACTIVATION_ENV;
    let pass = |value: &Option<String>| value.clone().filter(|_| !plan.background);
    LaunchCommand {
        argv: std::iter::once(command.program.clone())
            .chain(command.args.iter().cloned())
            .collect(),
        env: vec![
            (token_name.to_owned(), pass(&activation.token)),
            (startup_name.to_owned(), pass(&activation.startup_id)),
        ],
        desktop_id: plan.target.desktop_id().map(|id| id.as_str().to_owned()),
        name: plan.name.clone(),
    }
}

/// LAUNCH-07: notify with a button per alternative, remember what each
/// button opens, and fail the call.
async fn failed(
    ctx: &ServiceContext,
    plan: Plan,
    reason: String,
    activation: &Activation,
) -> Result<()> {
    tracing::warn!(target = %plan.target, %reason, "launch failed");
    // A start without a link (TRAY-20) has no URL line.
    let body = if plan.url.is_empty() {
        reason.clone()
    } else {
        format!("{reason}\n{}", plan.url)
    };
    let notification = Notification {
        summary: format!("Couldn't open {}", plan.name),
        body,
        actions: actions(&plan.alternatives),
        ..Notification::default()
    };
    match ctx.platform().notifier.notify(&notification).await {
        Ok(id) if !plan.alternatives.is_empty() => ctx.link().remember_failure(
            id,
            FailedLaunch {
                url: plan.url.clone(),
                alternatives: plan
                    .alternatives
                    .iter()
                    .map(|alt| alt.target.clone())
                    .collect(),
                activation: activation.clone(),
            },
        ),
        Ok(_) => {}
        Err(error) => tracing::warn!(%error, "cannot show the launch failure"),
    }
    Err(Error::failed(format!(
        "cannot open {}: {reason}",
        plan.name
    )))
}

fn actions(alternatives: &[Alternative]) -> Vec<(String, String)> {
    alternatives
        .iter()
        .enumerate()
        .map(|(index, alternative)| {
            (
                format!("{OPEN_ACTION}{index}"),
                format!("Open in {}", alternative.name),
            )
        })
        .collect()
}

/// PIPE-02: tell the user why the link went nowhere.
pub(crate) async fn notify_rejected(ctx: &ServiceContext, url: &str, rejected: &Rejected) {
    let notification = Notification {
        summary: "Wye can't open this link".to_owned(),
        body: format!("{rejected}\n{url}"),
        ..Notification::default()
    };
    if let Err(error) = ctx.platform().notifier.notify(&notification).await {
        tracing::warn!(%error, %rejected, "cannot show the rejected link");
    }
}

#[cfg(test)]
mod tests {
    use wye_core::{DesktopId, Target};

    use super::*;

    fn plan(background: bool) -> Plan {
        Plan {
            target: Target::App(DesktopId::new("firefox.desktop").expect("id")),
            name: "Firefox".into(),
            url: "https://example.com/".into(),
            background,
            command: Ok(wye_desktop::LaunchCommand {
                program: "firefox".into(),
                args: vec!["https://example.com/".into()],
                remove_env: Vec::new(),
            }),
            alternatives: Vec::new(),
            history: None,
        }
    }

    fn activation() -> Activation {
        Activation {
            token: Some("token".into()),
            startup_id: Some("startup".into()),
        }
    }

    #[test]
    fn the_activation_token_travels_with_the_launch() {
        let plan = plan(false);
        let command = platform_command(
            &plan,
            plan.command.as_ref().expect("command"),
            &activation(),
        );
        assert_eq!(command.argv, ["firefox", "https://example.com/"]);
        assert_eq!(
            command.env,
            [
                ("XDG_ACTIVATION_TOKEN".to_owned(), Some("token".to_owned())),
                ("DESKTOP_STARTUP_ID".to_owned(), Some("startup".to_owned())),
            ]
        );
        assert_eq!(command.desktop_id.as_deref(), Some("firefox.desktop"));
    }

    #[test]
    fn a_background_launch_takes_no_token() {
        // LAUNCH-04: without the token the compositor does not raise it.
        let plan = plan(true);
        let command = platform_command(
            &plan,
            plan.command.as_ref().expect("command"),
            &activation(),
        );
        assert!(command.env.iter().all(|(_, value)| value.is_none()));
    }

    #[test]
    fn each_alternative_gets_a_button() {
        let alternatives = [Alternative {
            target: Target::Picker,
            name: "Chromium".into(),
        }];
        assert_eq!(
            actions(&alternatives),
            [("open:0".to_owned(), "Open in Chromium".to_owned())]
        );
    }
}
