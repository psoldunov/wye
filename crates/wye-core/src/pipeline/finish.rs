//! Transform scripts and the last pipeline stage (PIPE-05, PIPE-13,
//! PIPE-14): everything that runs a [`Transformer`](crate::hooks::Transformer).

use url::Url;

use super::{Decision, LinkRequest, OpenOptions, Pipeline, Resolution, ScriptScope, Step};
use crate::hooks::{Hooks, RuleRef, ScriptError, TransformContext, Transformer};
use crate::target::Target;

/// What the user picked in the picker (PIPE-13). Held picker modifiers
/// (KEY-13) arrive as already applied: a private window is a
/// [`Target::Private`], background and new window are `options`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chosen {
    pub target: Target,
    pub options: OpenOptions,
}

/// A link ready to launch (PIPE-15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finished {
    /// The link after every script.
    pub url: Url,
    /// A concrete app, or [`Target::Picker`] when the picker was needed and
    /// nothing was chosen (PKS-07 holds it until the unlock).
    pub target: Target,
    pub options: OpenOptions,
    /// [`Resolution::steps`] followed by what `finish` did.
    pub steps: Vec<Step>,
}

impl Pipeline {
    /// PIPE-13 and PIPE-14: takes the user's choice from the picker (`None`
    /// when no picker was involved) and runs the matched rule's transform
    /// script. The script sees the link after the choice, so it can react to
    /// the source app and the held keys of the original request.
    ///
    /// Options the rule set and options the user chose are combined: either
    /// one turns background or new window on.
    #[must_use]
    pub fn finish(
        &self,
        resolution: &Resolution,
        request: &LinkRequest,
        chosen: Option<Chosen>,
        hooks: Hooks<'_>,
    ) -> Finished {
        let mut steps = resolution.steps.clone();
        let (target, options) = match chosen {
            Some(chosen) => {
                steps.push(Step::PickerChoice {
                    target: chosen.target.clone(),
                });
                (
                    chosen.target,
                    OpenOptions {
                        background: resolution.options.background || chosen.options.background,
                        new_window: resolution.options.new_window || chosen.options.new_window,
                    },
                )
            }
            None => (resolution.target.clone(), resolution.options),
        };
        let url = self.transform_rule(resolution, request, hooks, &mut steps);
        Finished {
            url,
            target,
            options,
            steps,
        }
    }

    // PIPE-05
    pub(super) fn transform_global(
        &self,
        url: Url,
        request: &LinkRequest,
        hooks: Hooks<'_>,
        steps: &mut Vec<Step>,
    ) -> Url {
        if !self.config.advanced.transform {
            return url;
        }
        let Some(transformer) = hooks.transformer() else {
            steps.push(Step::ScriptNotRun(ScriptScope::Global));
            return url;
        };
        let context = TransformContext {
            entry: request.entry,
            source: &request.source,
            held: request.held,
            rule: None,
        };
        run_script(transformer, ScriptScope::Global, url, &context, steps)
    }

    // PIPE-14
    fn transform_rule(
        &self,
        resolution: &Resolution,
        request: &LinkRequest,
        hooks: Hooks<'_>,
        steps: &mut Vec<Step>,
    ) -> Url {
        let url = resolution.url.clone();
        let Decision::Rule { index, name, .. } = &resolution.decision else {
            return url;
        };
        let Some(rule) = self.config.rules.get(*index).filter(|rule| rule.transform) else {
            return url;
        };
        let Some(transformer) = hooks.transformer() else {
            // `resolve` already recorded this when it had no transformer; a
            // caller that adds one only here gets the record now.
            if !steps.contains(&Step::ScriptNotRun(ScriptScope::Rule)) {
                steps.push(Step::ScriptNotRun(ScriptScope::Rule));
            }
            return url;
        };
        let context = TransformContext {
            entry: request.entry,
            source: &request.source,
            held: request.held,
            rule: Some(RuleRef {
                id: rule.id.as_deref(),
                name,
            }),
        };
        run_script(transformer, ScriptScope::Rule, url, &context, steps)
    }
}

/// Runs one script and records the outcome. A failure never stops the link
/// (SCR-22).
fn run_script(
    transformer: &dyn Transformer,
    scope: ScriptScope,
    url: Url,
    context: &TransformContext<'_>,
    steps: &mut Vec<Step>,
) -> Url {
    let outcome = transformer
        .transform(scope, &url, context)
        .and_then(|replacement| {
            replacement
                .map(|new| accept_replacement(&url, new))
                .transpose()
        });
    match outcome {
        Ok(Some(new)) if new != url => {
            steps.push(Step::Transformed {
                scope,
                url: new.clone(),
            });
            new
        }
        Ok(_) => {
            steps.push(Step::ScriptUnchanged(scope));
            url
        }
        Err(error) => {
            steps.push(Step::ScriptFailed {
                scope,
                message: error.to_string(),
            });
            url
        }
    }
}

/// SCR-23: a replacement must be an `http`/`https` link with a host. Handing
/// back the link that came in is fine, which matters for the local HTML file
/// links the global script also sees (DEF-07).
fn accept_replacement(original: &Url, replacement: Url) -> Result<Url, ScriptError> {
    let web = super::is_web(&replacement) && replacement.has_host();
    if web || replacement == *original {
        Ok(replacement)
    } else {
        Err(ScriptError::new(format!(
            "the script returned {replacement}, which is not an http or https link"
        )))
    }
}
