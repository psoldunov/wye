//! The first-run footer (ONB-06): **Back** at the start on every step after
//! the first, the progress dots in the middle, and the step's button at the
//! end: **Get Started**, **Continue** or **Done**, in the accent colour. The
//! default-browser step adds **Skip** beside it, and while Wye is not the
//! default there its **Make Default** is the suggested action instead.
//!
//! KDE counterpart: crates/wye-ui/qml/onboarding/OnboardingFooter.qml.

use adw::prelude::*;

use super::flow::Step;
use super::view::View;

/// The footer's widgets.
#[derive(Debug, Clone)]
pub struct Footer {
    pub bar: gtk::CenterBox,
    pub back: gtk::Button,
    pub skip: gtk::Button,
    pub next: gtk::Button,
    dots: gtk::Box,
}

impl Footer {
    pub fn new() -> Self {
        let back = gtk::Button::builder()
            .label("Back")
            .tooltip_text("Previous step")
            .build();
        let skip = gtk::Button::builder()
            .label("Skip")
            .tooltip_text("Keep the current default browser")
            .build();
        skip.add_css_class("flat");
        let next = gtk::Button::builder().label("Get Started").build();
        let end = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        end.append(&skip);
        end.append(&next);
        let dots = gtk::Box::builder()
            .spacing(8)
            .valign(gtk::Align::Center)
            .accessible_role(gtk::AccessibleRole::Img)
            .build();
        for _ in Step::ALL {
            let dot = gtk::Box::builder().valign(gtk::Align::Center).build();
            dot.add_css_class("wye-onboarding-dot");
            dots.append(&dot);
        }
        let bar = gtk::CenterBox::builder()
            .start_widget(&back)
            .center_widget(&dots)
            .end_widget(&end)
            .build();
        bar.add_css_class("wye-onboarding-footer");
        Self {
            bar,
            back,
            skip,
            next,
            dots,
        }
    }

    /// Follow `view`: which buttons show, what the step's button says, and
    /// which dot is lit.
    pub fn show(&self, view: &View) {
        let on_default_step = view.step == Step::DefaultBrowser;
        self.back.set_visible(view.can_go_back);
        self.skip.set_visible(on_default_step && !view.is_default);
        let label = if view.step == Step::Welcome {
            "Get Started"
        } else if view.is_last {
            "Done"
        } else {
            "Continue"
        };
        self.next.set_label(label);
        // While Wye is not the default, Make Default is the step's primary
        // button (ONB-02).
        let suggested = !on_default_step || view.is_default;
        if suggested {
            self.next.add_css_class("suggested-action");
        } else {
            self.next.remove_css_class("suggested-action");
        }
        let mut dot = self.dots.first_child();
        let mut index = 0;
        while let Some(widget) = dot {
            if index == view.step_index {
                widget.add_css_class("wye-onboarding-dot-current");
            } else {
                widget.remove_css_class("wye-onboarding-dot-current");
            }
            index += 1;
            dot = widget.next_sibling();
        }
        let step = format!("Step {} of {}", view.step_index + 1, view.step_count);
        self.dots
            .update_property(&[gtk::accessible::Property::Label(&step)]);
    }
}
