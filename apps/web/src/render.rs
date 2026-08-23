use astra_core::Theme;
use astra_engine::{FillRole, Scene, StrokeRole};
use leptos::prelude::*;

#[component]
#[allow(clippy::too_many_lines)]
pub fn WheelScene(scene: Memo<Result<Scene, String>>, theme: RwSignal<Theme>) -> impl IntoView {
    view! {
        <svg
            class="wheel"
            viewBox="0 0 400 400"
            role="img"
            aria-labelledby="wheel-title wheel-description"
        >
            <title id="wheel-title">"Deterministic demonstration chart"</title>
            <desc id="wheel-description">
                "A radial chart whose point markers and aspect lines are generated from a test ephemeris."
            </desc>
            {move || {
                let current_theme = theme.get();
                match scene.get() {
                    Ok(scene) => {
                        let circles = scene
                            .circles
                            .into_iter()
                            .map(|circle| {
                                let stroke = stroke_color(circle.stroke, &current_theme).to_owned();
                                let fill = fill_color(circle.fill, &current_theme).to_owned();
                                view! {
                                    <circle
                                        cx=circle.cx
                                        cy=circle.cy
                                        r=circle.radius
                                        stroke=stroke
                                        fill=fill
                                        stroke-width="1.5"
                                    />
                                }
                            })
                            .collect_view();
                        let lines = scene
                            .lines
                            .into_iter()
                            .map(|line| {
                                let stroke = stroke_color(line.stroke, &current_theme).to_owned();
                                view! {
                                    <line
                                        x1=line.x1
                                        y1=line.y1
                                        x2=line.x2
                                        y2=line.y2
                                        stroke=stroke
                                        stroke-width=line.width
                                    />
                                }
                            })
                            .collect_view();
                        let paths = scene
                            .paths
                            .into_iter()
                            .map(|path| {
                                let stroke = stroke_color(path.stroke, &current_theme).to_owned();
                                let fill = fill_color(path.fill, &current_theme).to_owned();
                                view! { <path d=path.data stroke=stroke fill=fill /> }
                            })
                            .collect_view();
                        let glyphs = scene
                            .glyphs
                            .into_iter()
                            .map(|glyph| {
                                let fill = fill_color(glyph.fill, &current_theme).to_owned();
                                view! {
                                    <text x=glyph.x y=glyph.y fill=fill text-anchor="middle">
                                        {glyph.value}
                                    </text>
                                }
                            })
                            .collect_view();
                        let labels = scene
                            .labels
                            .into_iter()
                            .map(|label| {
                                let fill = fill_color(label.fill, &current_theme).to_owned();
                                view! {
                                    <text
                                        x=label.x
                                        y=label.y
                                        fill=fill
                                        text-anchor="middle"
                                        dominant-baseline="middle"
                                    >
                                        {label.text}
                                    </text>
                                }
                            })
                            .collect_view();
                        view! { <g>{circles}{lines}{paths}{glyphs}{labels}</g> }.into_any()
                    }
                    Err(message) => view! {
                        <text x="200" y="200" text-anchor="middle" fill="#dc2626">
                            {message}
                        </text>
                    }
                    .into_any(),
                }
            }}
        </svg>
    }
}

fn stroke_color(role: StrokeRole, theme: &Theme) -> &str {
    match role {
        StrokeRole::Foreground => &theme.foreground,
        StrokeRole::Muted => &theme.muted,
        StrokeRole::Accent => &theme.accent,
        StrokeRole::Aspect => &theme.aspect_color,
    }
}

fn fill_color(role: FillRole, theme: &Theme) -> &str {
    match role {
        FillRole::None => "none",
        FillRole::Background => &theme.background,
        FillRole::Foreground => &theme.foreground,
        FillRole::Accent => &theme.accent,
    }
}
