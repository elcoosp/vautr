//! Phase 5 UI state primitives — Empty / Loading / Error / Success.
//!
//! Single canonical visuals for the transient states every list, form, and
//! async action can be in. Centralizing them here (instead of ad-hoc inline
//! `div()`s scattered through `desktop_view.rs`) guarantees that an empty
//! Secrets list looks the same as an empty Projects list, and that an error
//! surfaced in one section matches every other section.
//!
//! These mirror the web / extension / mobile canons:
//!   - `empty_state`   ≈ web `<EmptyState>` (icon + title + subtitle + CTA)
//!   - `loading_state` ≈ extension `Loader2` spinner
//!   - `error_callout` ≈ extension `border-destructive/40 bg-destructive/10`
//!   - `success_callout` ported from the desktop offboard/token result copy
//!
//! Phase 6 motion helpers (fade-in) live at the bottom of this file.

use gpui::*;
use gpui_component::{Icon, IconName, h_flex, v_flex};
use std::time::Duration;

use crate::theme;

/// Centered empty-state: icon + title + subtitle + optional action element.
pub fn empty_state(
    icon: IconName,
    title: impl Into<SharedString>,
    subtitle: impl Into<SharedString>,
    cta: Option<AnyElement>,
) -> impl IntoElement {
    let mut el = v_flex()
        .w_full()
        .py_10()
        .items_center()
        .gap_3()
        .child(Icon::new(icon).size_8().text_color(theme::TEXT_MUTED))
        .child(
            v_flex()
                .items_center()
                .gap_1()
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme::TEXT)
                        .child(title.into()),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme::TEXT_MUTED)
                        .child(subtitle.into()),
                ),
        );
    if let Some(cta) = cta {
        el = el.child(cta);
    }
    el
}

/// Inline loading state: spinner glyph + caption. Used for section loads and
/// in-flight actions (the submit button caption is driven separately by
/// `FormState::Submitting`).
pub fn loading_state(caption: impl Into<SharedString>) -> impl IntoElement {
    h_flex()
        .items_center()
        .gap_2()
        .text_color(theme::TEXT_MUTED)
        .child(
            Icon::new(IconName::Loader)
                .size_4()
                .text_color(theme::ACCENT),
        )
        .child(div().text_sm().child(caption.into()))
}

/// A single skeleton placeholder row (shimmer-less bar) for list skeletons.
pub fn skeleton_row() -> impl IntoElement {
    div().h_5().w_full().rounded_md().bg(theme::SURFACE_RAISED)
}

/// A vertical stack of `count` skeleton rows, for skeleton-loading lists.
pub fn skeleton_list(count: usize) -> impl IntoElement {
    let mut stack = v_flex().w_full().gap_2().py_2();
    for _ in 0..count {
        stack = stack.child(skeleton_row());
    }
    stack
}

/// Destructive-tinted error callout (icon + message). Mirrors the extension's
/// `border-destructive/40 bg-destructive/10` `ErrorCallout`.
pub fn error_callout(message: impl Into<SharedString>) -> impl IntoElement {
    h_flex()
        .w_full()
        .items_start()
        .gap_2()
        .border_1()
        .border_color(theme::DANGER)
        .rounded_md()
        .bg(theme::DANGER_BG)
        .px_3()
        .py_2()
        .child(
            Icon::new(IconName::TriangleAlert)
                .size_4()
                .text_color(theme::DANGER),
        )
        .child(
            div()
                .text_sm()
                .text_color(theme::DANGER_TEXT)
                .child(message.into()),
        )
}

/// Success callout (icon + message). Used for one-shot confirmations such as
/// an offboard result or a created access token.
pub fn success_callout(message: impl Into<SharedString>) -> impl IntoElement {
    h_flex()
        .w_full()
        .items_start()
        .gap_2()
        .border_1()
        .border_color(theme::ACCENT_DIM)
        .rounded_md()
        .bg(theme::SURFACE_RAISED)
        .px_3()
        .py_2()
        .child(
            Icon::new(IconName::Check)
                .size_4()
                .text_color(theme::ACCENT),
        )
        .child(
            div()
                .text_sm()
                .text_color(theme::TEXT)
                .child(message.into()),
        )
}

/// Phase 6 motion: wrap an element so it fades in (opacity 0 → 1) over the
/// canon `base` duration (150ms) using the `standard` easing curve
/// (`ease_in_out`). Used for dialog open and toast slide-in.
///
/// NOTE: this GPUI version's `Div` has no scale/transform API, so the canon's
/// "zoom 95% → 100%" part is not reproducible here; fade is the supported,
/// verifiable subset. Secret values and top-level section nav intentionally
/// do NOT use this (they must appear instantly per the motion canon).
pub fn fade_in<E: IntoElement + Styled + 'static>(el: E) -> impl IntoElement {
    el.with_animation(
        "ui-fade-in",
        Animation::new(Duration::from_millis(150)).with_easing(ease_in_out),
        |el, t| el.opacity(t),
    )
}
