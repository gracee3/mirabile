use leptos::prelude::*;
use mirabile_app::{AspectClass, AspectVisualStyle, FillRole, Scene, StrokeRole};

#[component]
#[allow(clippy::too_many_lines)]
pub fn WheelScene(scene: Scene, title: String, description: &'static str) -> impl IntoView {
    let width = if scene.width > 0.0 {
        scene.width
    } else {
        400.0
    };
    let height = if scene.height > 0.0 {
        scene.height
    } else {
        400.0
    };
    let view_box = format!("0 0 {width} {height}");
    let theme_style = scene.theme.as_ref().map(|theme| {
        format!(
            "--scene-background:{};--scene-foreground:{};--scene-muted:{};--scene-accent:{};--scene-aspect:{};background:{};color:{}",
            theme.background,
            theme.foreground,
            theme.muted,
            theme.accent,
            theme.aspect_color,
            theme.background,
            theme.foreground,
        )
    });

    let circles = scene
        .circles
        .into_iter()
        .map(|circle| {
            view! {
                <circle cx=circle.cx cy=circle.cy r=circle.radius
                    class=format!("{} {}", stroke_class(circle.stroke), fill_class(circle.fill))
                    stroke-width="1.5" />
            }
        })
        .collect_view();
    let lines = scene
        .lines
        .into_iter()
        .map(|line| {
            view! {
                <line x1=line.x1 y1=line.y1 x2=line.x2 y2=line.y2
                    class=stroke_class(line.stroke) stroke-width=line.width />
            }
        })
        .collect_view();
    let paths = scene.paths.into_iter().map(|path| view! {
        <path d=path.data class=format!("{} {}", stroke_class(path.stroke), fill_class(path.fill)) />
    }).collect_view();
    let glyphs = scene.glyphs.into_iter().map(|glyph| view! {
        <text x=glyph.x y=glyph.y class=fill_class(glyph.fill) text-anchor="middle">{glyph.value}</text>
    }).collect_view();
    let labels = scene
        .labels
        .into_iter()
        .map(|label| {
            view! {
                <text x=label.x y=label.y class=fill_class(label.fill) text-anchor="middle"
                    dominant-baseline="middle">{label.text}</text>
            }
        })
        .collect_view();

    let zodiac = scene.zodiac.into_iter().map(|sign| {
        let accessible_name = format!("{} zodiac sign", sign.name);
        view! {
            <g data-zodiac-sign=sign.id aria-label=accessible_name role="group">
                {sign.show_boundary.then(|| view! {
                    <line x1=sign.line.x1 y1=sign.line.y1 x2=sign.line.x2 y2=sign.line.y2
                        class="zodiac-boundary" />
                })}
                {sign.show_label.then(|| view! {
                    <text x=sign.label_x y=sign.label_y class="zodiac-glyph" text-anchor="middle"
                        dominant-baseline="middle" aria-label=sign.name>{sign.glyph}</text>
                })}
            </g>
        }
    }).collect_view();

    let houses = scene.houses.into_iter().map(|house| {
        let number = house.number.to_string();
        let accessible_name = format!(
            "House {} cusp at {:.4} degrees", house.number, house.cusp_longitude_degrees
        );
        view! {
            <g data-house=number.clone() aria-label=accessible_name role="group">
                {house.show_cusp.then(|| view! {
                    <line x1=house.line.x1 y1=house.line.y1 x2=house.line.x2 y2=house.line.y2
                        class="house-cusp" />
                })}
                {house.show_number.then(|| view! {
                    <text x=house.number_x y=house.number_y class="house-number" text-anchor="middle"
                        dominant-baseline="middle">{number.clone()}</text>
                })}
            </g>
        }
    }).collect_view();

    let angles = scene.angles.into_iter().map(|angle| {
        let accessible_name = format!(
            "{} at {:.4} degrees{}", angle.name, angle.longitude_degrees,
            if angle.derived_opposite { ", derived opposite" } else { "" }
        );
        view! {
            <g data-angle=angle.id data-derived-opposite=angle.derived_opposite.to_string()
                aria-label=accessible_name role="group">
                <line x1=angle.line.x1 y1=angle.line.y1 x2=angle.line.x2 y2=angle.line.y2
                    class="chart-angle-line" />
                <text x=angle.label_x y=angle.label_y class="chart-angle-label" text-anchor="middle"
                    dominant-baseline="middle">{angle.abbreviation}</text>
            </g>
        }
    }).collect_view();

    let aspects = scene.aspects.into_iter().map(|aspect| {
        let class_name = format!(
            "aspect-chord aspect-style-{} aspect-class-{}",
            aspect_style_name(aspect.style), aspect_class_name(aspect.classification)
        );
        let accessible_name = format!(
            "{} aspect from {} to {}, separation {:.3} degrees, orb {:.3} degrees{}",
            aspect.name, aspect.lhs, aspect.rhs, aspect.separation_degrees, aspect.orb_degrees,
            applying_suffix(aspect.applying)
        );
        view! {
            <g data-aspect-id=aspect.aspect_id data-aspect-lhs=aspect.lhs.to_string()
                data-aspect-rhs=aspect.rhs.to_string()
                data-aspect-lhs-slot=aspect.lhs_slot.as_ref().map(ToString::to_string)
                data-aspect-rhs-slot=aspect.rhs_slot.as_ref().map(ToString::to_string)
                data-aspect-layer=aspect.layer.as_str()
                data-aspect-classification=aspect_class_name(aspect.classification)
                data-aspect-applying=applying_value(aspect.applying)
                data-aspect-chord=aspect.draw_chord.to_string()
                aria-label=accessible_name role="group">
                {aspect.draw_chord.then(|| view! {
                    <line x1=aspect.x1 y1=aspect.y1 x2=aspect.x2 y2=aspect.y2 class=class_name />
                })}
            </g>
        }
    }).collect_view();

    let points = scene.points.into_iter().map(|point| {
        let point_id = point.point.to_string();
        let accessible_name = format!(
            "{} at {}, longitude {:.4} degrees, latitude {:.4} degrees{}",
            point.name, point.formatted_position, point.longitude_degrees, point.latitude_degrees,
            if point.retrograde { ", retrograde" } else { "" }
        );
        let retrograde_marker = point.retrograde && point.show_retrograde;
        let text_anchor = label_anchor(point.label_x, width);
        let retrograde_x = match text_anchor {
            "start" => point.label_x + point.label_width + 4.0,
            "end" => point.label_x - point.label_width - 4.0,
            _ => point.label_x + point.label_width / 2.0 + 4.0,
        };
        view! {
            <g data-point-id=point_id data-point-retrograde=point.retrograde.to_string()
                data-chart-slot=point.chart_slot.as_ref().map(ToString::to_string)
                data-ring-role=point.ring_role.map(|role| format!("{role:?}").to_lowercase())
                data-glyph-fallback=point.glyph_fallback.to_string()
                aria-label=accessible_name role="group">
                <circle cx=point.x cy=point.y r="3.5" class="point-true-anchor"
                    data-point-anchor="true" />
                {point.leader.map(|leader| view! {
                    <line x1=leader.x1 y1=leader.y1 x2=leader.x2 y2=leader.y2
                        class="point-leader" data-point-leader="true" />
                })}
                <text x=point.label_x y=point.label_y class="point-label" text-anchor=text_anchor
                    dominant-baseline="middle" data-point-label="true">{point.display_label}</text>
                {retrograde_marker.then(|| view! {
                    <text x=retrograde_x y=point.label_y class="retrograde-marker"
                        text-anchor=text_anchor dominant-baseline="middle"
                        data-retrograde-marker="true" aria-label="retrograde">"℞"</text>
                })}
            </g>
        }
    }).collect_view();

    view! {
        <svg class="wheel-scene" viewBox=view_box role="img"
            aria-labelledby="active-scene-title active-scene-description"
            preserveAspectRatio="xMidYMid meet" data-scene-width=width.to_string()
            data-scene-height=height.to_string() style=theme_style>
            <title id="active-scene-title">{title}</title>
            <desc id="active-scene-description">{description}</desc>
            <g data-wheel-group="compatibility-primitives" aria-hidden="true">
                {circles}{lines}{paths}{glyphs}{labels}
            </g>
            <g data-wheel-group="zodiac" aria-label="Zodiac signs">{zodiac}</g>
            <g data-wheel-group="houses" aria-label="Calculated houses">{houses}</g>
            <g data-wheel-group="aspects" aria-label="Calculated aspects">{aspects}</g>
            <g data-wheel-group="angles" aria-label="Chart angles">{angles}</g>
            <g data-wheel-group="points" aria-label="Calculated points">{points}</g>
        </svg>
    }
}

const fn label_anchor(label_x: f64, width: f64) -> &'static str {
    if label_x < width * 0.28 {
        "start"
    } else if label_x > width * 0.72 {
        "end"
    } else {
        "middle"
    }
}

const fn aspect_style_name(style: AspectVisualStyle) -> &'static str {
    match style {
        AspectVisualStyle::Conjunction => "conjunction",
        AspectVisualStyle::Opposition => "opposition",
        AspectVisualStyle::Square => "square",
        AspectVisualStyle::Trine => "trine",
        AspectVisualStyle::Sextile => "sextile",
        AspectVisualStyle::Quincunx => "quincunx",
        AspectVisualStyle::Neutral => "neutral",
    }
}

const fn aspect_class_name(classification: AspectClass) -> &'static str {
    match classification {
        AspectClass::Major => "major",
        AspectClass::Minor => "minor",
        AspectClass::Harmonic => "harmonic",
        AspectClass::Custom => "custom",
    }
}

const fn applying_value(applying: Option<bool>) -> &'static str {
    match applying {
        Some(true) => "true",
        Some(false) => "false",
        None => "unknown",
    }
}

const fn applying_suffix(applying: Option<bool>) -> &'static str {
    match applying {
        Some(true) => ", applying",
        Some(false) => ", separating",
        None => ", applying state unavailable",
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
