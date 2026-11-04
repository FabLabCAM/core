use super::*;
const EPS: f64 = 1e-7;
pub fn connections(parts: &[Panel]) -> Vec<Connection> {
    let mut result = Vec::new();
    for male in parts {
        for female in parts {
            if male.id == female.id
                || male.normal == female.normal
                || male.assembly != female.assembly
            {
                continue;
            }
            let axis = female.normal;
            let low = (male.origin[axis] - female.origin[axis] - female.size[axis]).abs() < EPS;
            let high = (male.origin[axis] + male.size[axis] - female.origin[axis]).abs() < EPS;
            if !low && !high {
                continue;
            }
            let span_axis = 3 - axis - male.normal;
            let lo = male.origin[span_axis].max(female.origin[span_axis]);
            let hi = (male.origin[span_axis] + male.size[span_axis])
                .min(female.origin[span_axis] + female.size[span_axis]);
            let n = male.normal;
            if hi - lo <= EPS
                || male.origin[n] + male.size[n] <= female.origin[n] + EPS
                || male.origin[n] >= female.origin[n] + female.size[n] - EPS
            {
                continue;
            }
            result.push(Connection {
                id: format!(
                    "{}--{}-{}-{}",
                    male.id,
                    female.id,
                    axis,
                    if high { "hi" } else { "lo" }
                ),
                male: male.id.clone(),
                female: female.id.clone(),
                axis,
                high_end: high,
                female_face: if high {
                    MachiningFace::Front
                } else {
                    MachiningFace::Back
                },
                span_axis,
                span: [lo, hi],
            });
        }
    }
    result.sort_by(|a, b| a.id.cmp(&b.id));
    result
}
