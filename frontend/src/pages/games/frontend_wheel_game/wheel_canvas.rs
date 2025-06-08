use yew::prelude::*;
use web_sys::{window, HtmlCanvasElement, CanvasRenderingContext2d};
use wasm_bindgen::JsCast;
use std::f64::consts::PI;

#[derive(Properties, PartialEq)]
pub struct WheelCanvasProps {
    pub rotation: f64,
    pub is_spinning: bool,
    pub will_win: bool,
}

#[function_component(WheelCanvas)]
pub fn wheel_canvas(props: &WheelCanvasProps) -> Html {
    let canvas_ref = use_node_ref();
    
    {
        let canvas_ref = canvas_ref.clone();
        let rotation = props.rotation;
        let will_win = props.will_win;
        let is_spinning = props.is_spinning;
        
        use_effect_with(
            (rotation, will_win, is_spinning),
            move |(rotation, _will_win, is_spinning)| {
                if let Some(canvas) = canvas_ref.cast::<HtmlCanvasElement>() {
                    let context = canvas
                        .get_context("2d")
                        .unwrap()
                        .unwrap()
                        .dyn_into::<CanvasRenderingContext2d>()
                        .unwrap();
                    
                    let width = canvas.width() as f64;
                    let height = canvas.height() as f64;
                    let center_x = width / 2.0;
                    let center_y = height / 2.0;
                    let radius = if width < height { width / 2.0 - 20.0 } else { height / 2.0 - 20.0 };
                    
                    // Clear canvas
                    context.clear_rect(0.0, 0.0, width, height);
                    
                    // Check if dark mode is active
                    let is_dark_mode = if let Some(window) = window() {
                        if let Some(document) = window.document() {
                            document.document_element()
                                .and_then(|el| Some(el.class_list().contains("dark")))
                                .unwrap_or(false)
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    
                    // Enhanced magical outer glow effect
                    let glow_radius = radius + 15.0;
                    let glow_intensity = if *is_spinning { 0.25 } else { 0.15 };
                    context.begin_path();
                    if is_dark_mode {
                        context.set_fill_style_str(&format!("rgba(130, 100, 255, {})", glow_intensity));
                    } else {
                        context.set_fill_style_str(&format!("rgba(100, 130, 255, {})", glow_intensity));
                    }
                    let _ = context.arc(center_x, center_y, glow_radius, 0.0, 2.0 * PI);
                    context.fill();
                    
                    // Add a second magical glow layer when spinning
                    if *is_spinning {
                        let pulse = (js_sys::Date::now() as f64 / 300.0).sin() * 0.1 + 0.2;
                        context.begin_path();
                        context.set_fill_style_str(&format!("rgba(255, 215, 130, {})", pulse));
                        let _ = context.arc(center_x, center_y, glow_radius + 5.0, 0.0, 2.0 * PI);
                        context.fill();
                    }
                    
                    // Draw wheel background with a subtle gradient
                    context.begin_path();
                    if is_dark_mode {
                        context.set_fill_style_str("#1a1c2e");
                    } else {
                        context.set_fill_style_str("#f0f2ff");
                    }
                    let _ = context.arc(center_x, center_y, radius, 0.0, 2.0 * PI);
                    context.fill();
                    
                    // Save context state before rotation
                    context.save();
                    
                    // Move to center, rotate, then move back
                    let _ = context.translate(center_x, center_y);
                    let _ = context.rotate(*rotation * PI / 180.0);
                    let _ = context.translate(-center_x, -center_y);
                    
                    // Modern magical color palette
                    let segment_colors = [
                        "#f97316", // Orange (Scroll)
                        "#06b6d4", // Cyan (50 PAX)
                        "#8b5cf6", // Violet (20 PAX)
                        "#ec4899", // Pink (10 PAX)
                    ];
                    
                    // Draw the four segments with modern magical colors
                    // First segment (orange for scroll win - 25%)
                    context.begin_path();
                    context.set_fill_style_str(segment_colors[0]);
                    context.move_to(center_x, center_y);
                    let _ = context.arc(center_x, center_y, radius, 0.0, 0.5 * PI);
                    context.fill();
                    
                    // Second segment (cyan for 50 pax win - 15%)
                    context.begin_path();
                    context.set_fill_style_str(segment_colors[1]);
                    context.move_to(center_x, center_y);
                    let _ = context.arc(center_x, center_y, radius, 0.5 * PI, 0.8 * PI);
                    context.fill();
                    
                    // Third segment (violet for 20 pax win - 25%)
                    context.begin_path();
                    context.set_fill_style_str(segment_colors[2]);
                    context.move_to(center_x, center_y);
                    let _ = context.arc(center_x, center_y, radius, 0.8 * PI, 1.3 * PI);
                    context.fill();
                    
                    // Fourth segment (pink for 10 pax win - 35%)
                    context.begin_path();
                    context.set_fill_style_str(segment_colors[3]);
                    context.move_to(center_x, center_y);
                    let _ = context.arc(center_x, center_y, radius, 1.3 * PI, 2.0 * PI);
                    context.fill();
                    
                    // Add magical shimmer effect to segments when spinning
                    if *is_spinning {
                        let shimmer_opacity = (js_sys::Date::now() as f64 / 400.0).sin() * 0.15 + 0.15;
                        
                        // First segment shimmer
                        context.begin_path();
                        context.set_fill_style_str(&format!("rgba(255, 255, 255, {})", shimmer_opacity));
                        context.move_to(center_x, center_y);
                        let _ = context.arc(center_x, center_y, radius, 0.0, 0.5 * PI);
                        context.fill();
                        
                        // Second segment shimmer
                        context.begin_path();
                        context.set_fill_style_str(&format!("rgba(255, 255, 255, {})", shimmer_opacity * 0.8));
                        context.move_to(center_x, center_y);
                        let _ = context.arc(center_x, center_y, radius, 0.5 * PI, 0.8 * PI);
                        context.fill();
                        
                        // Third segment shimmer
                        context.begin_path();
                        context.set_fill_style_str(&format!("rgba(255, 255, 255, {})", shimmer_opacity * 0.6));
                        context.move_to(center_x, center_y);
                        let _ = context.arc(center_x, center_y, radius, 0.8 * PI, 1.3 * PI);
                        context.fill();
                        
                        // Fourth segment shimmer
                        context.begin_path();
                        context.set_fill_style_str(&format!("rgba(255, 255, 255, {})", shimmer_opacity * 0.7));
                        context.move_to(center_x, center_y);
                        let _ = context.arc(center_x, center_y, radius, 1.3 * PI, 2.0 * PI);
                        context.fill();
                    }
                    
                    // Draw segment dividers with enhanced glow
                    let divider_width = 2.5;
                    let divider_glow = if *is_spinning { 2.0 } else { 1.0 };
                    
                    // Draw dividing lines between segments with glow effect
                    for angle in [0.0, 0.5 * PI, 0.8 * PI, 1.3 * PI] {
                        // Draw glow
                        context.begin_path();
                        context.set_stroke_style_str(if is_dark_mode { 
                            "rgba(255, 255, 255, 0.3)" 
                        } else { 
                            "rgba(255, 255, 255, 0.7)" 
                        });
                        context.set_line_width(divider_width + divider_glow * 2.0);
                        context.move_to(center_x, center_y);
                        let end_x = center_x + radius * angle.cos();
                        let end_y = center_y + radius * angle.sin();
                        context.line_to(end_x, end_y);
                        context.stroke();
                        
                        // Draw main divider
                        context.begin_path();
                        context.set_stroke_style_str(if is_dark_mode { 
                            "rgba(255, 255, 255, 0.7)" 
                        } else { 
                            "rgba(255, 255, 255, 0.9)" 
                        });
                        context.set_line_width(divider_width);
                        context.move_to(center_x, center_y);
                        context.line_to(end_x, end_y);
                        context.stroke();
                    }
                    
                    // Draw inner circle to create a ring effect
                    let inner_radius = radius * 0.25;
                    
                    context.begin_path();
                    if is_dark_mode {
                        context.set_fill_style_str("#2d3142"); // Darker magical blue for dark mode
                    } else {
                        context.set_fill_style_str("#8b5cf6"); // Violet for light mode
                    }
                    let _ = context.arc(center_x, center_y, inner_radius, 0.0, 2.0 * PI);
                    context.fill();
                    
                    // Add a subtle inner shadow to the inner circle
                    context.begin_path();
                    context.set_stroke_style_str(
                        if is_dark_mode { "rgba(0, 0, 0, 0.5)" } else { "rgba(0, 0, 0, 0.2)" }
                    );
                    context.set_line_width(2.0);
                    let _ = context.arc(center_x, center_y, inner_radius, 0.0, 2.0 * PI);
                    context.stroke();
                    
                    // Add a decorative pattern or icon to the center
                    // Draw a magical star pattern
                    let star_points = 8;
                    let star_outer_radius = inner_radius * 0.7;
                    let star_inner_radius = inner_radius * 0.3;
                    
                    context.begin_path();
                    context.set_fill_style_str(if is_dark_mode { 
                        "#a78bfa" // Lighter violet for dark mode
                    } else { 
                        "#ffffff" // White for light mode
                    });
                    
                    for i in 0..star_points * 2 {
                        let angle = i as f64 * PI / star_points as f64;
                        let r = if i % 2 == 0 { star_outer_radius } else { star_inner_radius };
                        let x = center_x + r * angle.cos();
                        let y = center_y + r * angle.sin();
                        
                        if i == 0 {
                            context.move_to(x, y);
                        } else {
                            context.line_to(x, y);
                        }
                    }
                    
                    context.close_path();
                    context.fill();
                    
                    // Add a magical glow to the star when spinning
                    if *is_spinning {
                        context.begin_path();
                        let glow_opacity = (js_sys::Date::now() as f64 / 300.0).sin() * 0.2 + 0.3;
                        context.set_fill_style_str(&format!("rgba(255, 215, 130, {})", glow_opacity));
                        
                        for i in 0..star_points * 2 {
                            let angle = i as f64 * PI / star_points as f64;
                            let r = if i % 2 == 0 { 
                                star_outer_radius * 1.1 
                            } else { 
                                star_inner_radius * 1.1 
                            };
                            let x = center_x + r * angle.cos();
                            let y = center_y + r * angle.sin();
                            
                            if i == 0 {
                                context.move_to(x, y);
                            } else {
                                context.line_to(x, y);
                            }
                        }
                        
                        context.close_path();
                        context.fill();
                    }
                    
                    // Add a small circle in the very center
                    context.begin_path();
                    context.set_fill_style_str(if is_dark_mode { 
                        "#d8b4fe" // Light purple for dark mode
                    } else { 
                        "#ffffff" // White for light mode
                    });
                    let _ = context.arc(center_x, center_y, inner_radius * 0.15, 0.0, 2.0 * PI);
                    context.fill();
                    
                    // Draw segment labels with improved typography and positioning
                    // Use a more modern font stack and add text shadows for better readability
                    let base_font = "bold 20px 'Segoe UI', Roboto, system-ui, sans-serif";
                    
                    context.set_text_align("center");
                    context.set_text_baseline("middle");
                    
                    // Set text color with better contrast
                    context.set_fill_style_str("#ffffff");
                    
                    // Add text shadow for better readability
                    context.set_shadow_color(if is_dark_mode { "rgba(0, 0, 0, 0.7)" } else { "rgba(0, 0, 0, 0.5)" });
                    context.set_shadow_blur(3.0);
                    context.set_shadow_offset_x(1.0);
                    context.set_shadow_offset_y(1.0);
                    
                    // Draw reward text with better positioning - NO DOTS
                    // Scroll label
                    context.save();
                    let _ = context.translate(center_x, center_y);
                    let _ = context.rotate(0.25 * PI); // Middle of first segment
                    let _ = context.translate(radius * 0.6, 0.0);
                    context.set_font(base_font);
                    let _ = context.fill_text("SCROLL", 0.0, 0.0);
                    context.restore();
                    
                    // 50 Pax label
                    context.save();
                    let _ = context.translate(center_x, center_y);
                    let _ = context.rotate(0.65 * PI); // Middle of second segment
                    let _ = context.translate(radius * 0.6, 0.0);
                    context.set_font(base_font);
                    let _ = context.fill_text("50 pax", 0.0, 0.0);
                    context.restore();
                    
                    // 20 Pax label
                    context.save();
                    let _ = context.translate(center_x, center_y);
                    let _ = context.rotate(1.05 * PI); // Middle of third segment
                    let _ = context.translate(radius * 0.6, 0.0);
                    context.set_font(base_font);
                    let _ = context.fill_text("20 pax", 0.0, 0.0);
                    context.restore();
                    
                    // 10 PAX label
                    context.save();
                    let _ = context.translate(center_x, center_y);
                    let _ = context.rotate(1.65 * PI); // Middle of fourth segment
                    let _ = context.translate(radius * 0.6, 0.0);
                    context.set_font(base_font);
                    let _ = context.fill_text("10 pax", 0.0, 0.0);
                    context.restore();
                    
                    // Reset shadow for subsequent drawing
                    context.set_shadow_color("rgba(0, 0, 0, 0)");
                    context.set_shadow_blur(0.0);
                    context.set_shadow_offset_x(0.0);
                    context.set_shadow_offset_y(0.0);
                    
                    // Restore context to original state (no rotation)
                    context.restore();
                    
                    // Draw outer ring with enhanced magical effect
                    context.begin_path();
                    if *is_spinning {
                        // Animated gradient-like effect when spinning
                        let pulse = (js_sys::Date::now() as f64 / 400.0).sin() * 0.2 + 0.5;
                        let stroke_color = if is_dark_mode { 
                            format!("rgba(180, 130, 255, {})", pulse)
                        } else { 
                            format!("rgba(130, 100, 255, {})", pulse)
                        };
                        context.set_stroke_style_str(&stroke_color);
                        context.set_line_width(5.0);
                    } else {
                        context.set_stroke_style_str(
                            if is_dark_mode { 
                                "rgba(180, 130, 255, 0.5)" 
                            } else { 
                                "rgba(130, 100, 255, 0.5)" 
                            }
                        );
                        context.set_line_width(4.0);
                    }
                    let _ = context.arc(center_x, center_y, radius - 2.0, 0.0, 2.0 * PI);
                    context.stroke();
                    
                    // Draw a modern pointer with enhanced magical effects
                    // Create a glowing effect for the pointer when spinning
                    let pointer_glow_size = if *is_spinning { 10.0 } else { 4.0 };
                    context.set_shadow_color(if *is_spinning {
                        "rgba(255, 215, 130, 0.8)"
                    } else {
                        "rgba(255, 215, 0, 0.6)"
                    });
                    context.set_shadow_blur(pointer_glow_size);
                    context.set_shadow_offset_x(0.0);
                    context.set_shadow_offset_y(0.0);
                    
                    // Draw a more modern pointer shape
                    context.begin_path();
                    
                    // Create a rounded triangle pointer
                    let pointer_width = 20.0;
                    let pointer_height = 30.0;
                    let pointer_radius = 5.0; // Radius for rounded corners
                    
                    // Start at the bottom point
                    context.move_to(center_x, center_y - radius + 5.0);
                    
                    // Draw line to left corner with rounded edge
                    let left_corner_x = center_x - pointer_width;
                    let left_corner_y = center_y - radius - pointer_height;
                    context.line_to(left_corner_x + pointer_radius, left_corner_y + pointer_radius);
                    
                    // Draw rounded left corner
                    context.quadratic_curve_to(
                        left_corner_x, left_corner_y + pointer_radius,
                        left_corner_x, left_corner_y
                    );
                    
                    // Draw top line
                    context.line_to(center_x + pointer_width - pointer_radius, left_corner_y);
                    
                    // Draw rounded right corner
                    context.quadratic_curve_to(
                        center_x + pointer_width, left_corner_y,
                        center_x + pointer_width, left_corner_y + pointer_radius
                    );
                    
                    // Close the path
                    context.close_path();
                    
                    // Magical gold color for pointer
                    if *is_spinning {
                        context.set_fill_style_str("#ffd700");
                    } else {
                        context.set_fill_style_str("#f59e0b");
                    }
                    context.fill();
                    
                    // Add a subtle stroke to the pointer
                    context.set_stroke_style_str("#e69500");
                    context.set_line_width(1.5);
                    context.stroke();
                    
                    // Reset shadow
                    context.set_shadow_color("rgba(0, 0, 0, 0)");
                    context.set_shadow_blur(0.0);
                    
                    // Add a magical pulsing effect to the pointer when spinning
                    if *is_spinning {
                        // Draw a magical glow around the pointer
                        context.begin_path();
                        context.set_fill_style_str("rgba(255, 215, 130, 0.25)");
                        let pulse_size = 12.0 + (js_sys::Date::now() as f64 / 200.0).sin() * 6.0;
                        let _ = context.arc(center_x, center_y - radius + 5.0, pulse_size, 0.0, 2.0 * PI);
                        context.fill();
                        
                        // Add magical particles around the wheel when spinning
                        let time = js_sys::Date::now() as f64;
                        let num_particles = 12;
                        
                        for i in 0..num_particles {
                            let angle = (time / 1000.0 + i as f64 * 2.0 * PI / num_particles as f64) % (2.0 * PI);
                            let distance = radius * 1.1 + (time / 500.0 + i as f64).sin() * 10.0;
                            let x = center_x + distance * angle.cos();
                            let y = center_y + distance * angle.sin();
                            let size = 2.0 + (time / 300.0 + i as f64).sin() * 1.5;
                            
                            context.begin_path();
                            context.set_fill_style_str("rgba(255, 215, 130, 0.7)");
                            let _ = context.arc(x, y, size, 0.0, 2.0 * PI);
                            context.fill();
                        }
                    }
                }
                || ()
            }
        );
    }
    
    html! {
        <div class="relative">
            <canvas 
                ref={canvas_ref}
                width="450"
                height="450"
                class="w-full max-w-[450px] h-auto rounded-full shadow-lg transition-all duration-300"
                style={if props.is_spinning {
                    "filter: drop-shadow(0px 5px 20px rgba(130, 100, 255, 0.4));"
                } else {
                    "filter: drop-shadow(0px 5px 15px rgba(0, 0, 0, 0.2));"
                }}
            />
        </div>
    }
}

// Easing function for smooth deceleration
pub fn ease_out_cubic(t: f64) -> f64 {
    // Modified ease-out: 1 - (1-t)^4
    1.0 - (1.0 - t).powi(4)
} 