use astra_app::{FillRole, Scene, StrokeRole};
use leptos::prelude::*;

#[component]
#[allow(clippy::too_many_lines)]
pub fn WheelScene(scene: Scene, title: String, description: &'static str) -> impl IntoView {
    let circles = scene
        .circles
        .into_iter()
        .map(|circle| {
            view! {
                <circle
                    cx=circle.cx
                    cy=circle.cy
                    r=circle.radius
                    class=format!("{} {}", stroke_class(circle.stroke), fill_class(circle.fill))
                    stroke-width="1.5"
                />
            }
        })
        .collect_view();
    let lines = scene
        .lines
        .into_iter()
        .map(|line| {
            view! {
                <line
                    x1=line.x1
                    y1=line.y1
                    x2=line.x2
                    y2=line.y2
                    class=stroke_class(line.stroke)
                    stroke-width=line.width
                />
            }
        })
        .collect_view();
    let paths = scene
        .paths
        .into_iter()
        .map(|path| {
            view! {
                <path
                    d=path.data
                    class=format!("{} {}", stroke_class(path.stroke), fill_class(path.fill))
                />
            }
        })
        .collect_view();
    let glyphs = scene
        .glyphs
        .into_iter()
        .map(|glyph| {
            view! {
                <text
                    x=glyph.x
                    y=glyph.y
                    class=fill_class(glyph.fill)
                    text-anchor="middle"
                >
                    {glyph.value}
                </text>
            }
        })
        .collect_view();
    let labels = scene
        .labels
        .into_iter()
        .map(|label| {
            view! {
                <text
                    x=label.x
                    y=label.y
                    class=fill_class(label.fill)
                    text-anchor="middle"
                    dominant-baseline="middle"
                >
                    {label.text}
                </text>
            }
        })
        .collect_view();

    view! {
        <svg
            class="wheel-scene"
            viewBox="0 0 400 400"
            role="img"
            aria-labelledby="active-scene-title active-scene-description"
        >
            <title id="active-scene-title">{title}</title>
            <desc id="active-scene-description">{description}</desc>
            <g>{circles}{lines}{paths}{glyphs}{labels}</g>
        </svg>
    }
}

const fn stroke_class(role: StrokeRole) -> &'static str {
    match role {
        StrokeRole::Foreground => "scene-stroke-foreground",
        StrokeRole::Muted => "scene-stroke-muted",
        StrokeRole::Accent => "scene-stroke-accent",
        StrokeRole::Aspect => "scene-stroke-aspect",
    }
}

const fn fill_class(role: FillRole) -> &'static str {
    match role {
        FillRole::None => "scene-fill-none",
        FillRole::Background => "scene-fill-background",
        FillRole::Foreground => "scene-fill-foreground",
        FillRole::Accent => "scene-fill-accent",
    }
}
