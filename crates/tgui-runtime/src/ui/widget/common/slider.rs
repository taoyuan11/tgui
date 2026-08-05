pub(crate) fn slider_effective_step(min: f32, max: f32, step: f32) -> Option<f32> {
    if !step.is_finite() || step <= 0.0 {
        return None;
    }
    let range = (max - min).abs();
    if !range.is_finite() || range <= f32::EPSILON {
        return None;
    }
    Some(step.min(range))
}

pub(crate) fn slider_interaction_step(min: f32, max: f32, step: f32) -> Option<f32> {
    slider_effective_step(min, max, step).or_else(|| {
        let range = (max - min).abs();
        (range.is_finite() && range > f32::EPSILON).then_some(range / 100.0)
    })
}

pub(crate) fn slider_clamp_value(value: f32, min: f32, max: f32) -> f32 {
    if !value.is_finite() {
        min
    } else {
        value.clamp(min, max)
    }
}

pub(crate) fn slider_quantize_value(value: f32, min: f32, max: f32, step: f32) -> f32 {
    let clamped = slider_clamp_value(value, min, max);
    let Some(step) = slider_effective_step(min, max, step) else {
        return clamped;
    };
    let steps = ((clamped - min) / step).round();
    slider_clamp_value(min + (steps * step), min, max)
}

pub(crate) fn slider_resolve_value(value: f32, min: f32, max: f32, step: f32) -> f32 {
    slider_quantize_value(value, min, max, step)
}

pub(crate) fn slider_normalized_value(value: f32, min: f32, max: f32, step: f32) -> f32 {
    let range = max - min;
    if range.abs() <= f32::EPSILON {
        return 0.0;
    }
    ((slider_resolve_value(value, min, max, step) - min) / range).clamp(0.0, 1.0)
}

pub(crate) fn slider_value_from_normalized(normalized: f32, min: f32, max: f32, step: f32) -> f32 {
    let range = max - min;
    if range.abs() <= f32::EPSILON {
        return min;
    }
    slider_quantize_value(min + normalized.clamp(0.0, 1.0) * range, min, max, step)
}

pub(crate) fn slider_tick_count(min: f32, max: f32, step: f32, explicit: Option<usize>) -> usize {
    if let Some(explicit) = explicit {
        return explicit.max(2).min(101);
    }
    let Some(step) = slider_effective_step(min, max, step) else {
        return 2;
    };
    let count = (((max - min).abs() / step).round() as usize).saturating_add(1);
    count.max(2).min(101)
}
