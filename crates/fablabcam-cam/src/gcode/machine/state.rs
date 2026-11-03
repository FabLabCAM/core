use super::{Block, Point, Profile};

pub(crate) struct State {
    pub position: Option<Point>,
    pub scale: f64,
    pub absolute: bool,
    pub arc_absolute: bool,
    pub plane: [usize; 3],
    pub feed_mode: String,
    pub offset: Option<Point>,
    pub feed: Option<f64>,
    pub speed: f64,
    pub spindle: Option<&'static str>,
    pub motion: Option<u8>,
    pub poisoned: bool,
    pub ended: bool,
}
impl State {
    pub fn new(p: &Profile) -> Self {
        Self { position: p.initial_position_mm, scale: if p.defaults.units == "G20" {25.4} else {1.}, absolute: p.defaults.distance == "G90", arc_absolute: p.defaults.arc_distance == "G90.1", plane: plane(&p.defaults.plane), feed_mode: p.defaults.feed_mode.clone(), offset: p.work_offsets_mm.get(&p.defaults.work_offset).copied(), feed: None, speed: 0., spindle: None, motion: None, poisoned: false, ended: false }
    }
    pub fn modes(&mut self, b: &Block, p: &Profile) {
        let previous_units = self.scale;
        let previous_feed_mode = self.feed_mode.clone();
        for w in &b.words {
            match w.code().as_str() {
                "G20" | "G70" => self.scale = 25.4,
                "G21" | "G71" => self.scale = 1.,
                "G90" => self.absolute = true, "G91" => self.absolute = false,
                "G90.1" => self.arc_absolute = true, "G91.1" => self.arc_absolute = false,
                c @ ("G17" | "G18" | "G19") => self.plane = plane(c),
                c @ ("G93" | "G94" | "G95") => self.feed_mode = c.into(),
                c @ ("G54" | "G55" | "G56" | "G57" | "G58" | "G59") => self.offset = p.work_offsets_mm.get(c).copied(),
                "G0" => self.motion = Some(0), "G1" => self.motion = Some(1),
                "G2" => self.motion = Some(2), "G3" => self.motion = Some(3),
                "G80" => self.motion = None,
                "M3" => self.spindle = Some("cw"), "M4" => self.spindle = Some("ccw"), "M5" => self.spindle = None,
                _ => {}
            }
        }
        if previous_units != self.scale || previous_feed_mode != self.feed_mode { self.feed = None; }
        if let Some(f) = b.value('F') { self.feed = Some(if self.feed_mode == "G93" {f} else {f*self.scale}); }
        if let Some(s) = b.value('S') { self.speed = s; }
    }
}
fn plane(c: &str) -> [usize;3] {
    match c { "G18" => [2,0,1], "G19" => [1,2,0], _ => [0,1,2] }
}
/// Modal group membership from the Mill manual. Used to reject ambiguous blocks.
pub(crate) fn group(c: &str) -> Option<u8> {
    Some(match c {
        "G4"|"G10"|"G28"|"G28.1"|"G30"|"G52"|"G53"|"G92"|"G92.1"|"G92.2"|"G92.3" => 0,
        "G0"|"G1"|"G2"|"G3"|"G12"|"G13"|"G31"|"G73"|"G80"|"G81"|"G82"|"G83"|"G84"|"G85"|"G86"|"G87"|"G88"|"G89" => 1,
        "G17"|"G18"|"G19" => 2, "G90"|"G91" => 3, "G90.1"|"G91.1" => 4,
        "G93"|"G94"|"G95" => 5, "G20"|"G21"|"G70"|"G71" => 6,
        "G40"|"G41"|"G42" => 7, "G43"|"G49" => 8, "G98"|"G99" => 9,
        "G54"|"G55"|"G56"|"G57"|"G58"|"G59" => 10, "G61"|"G61.1"|"G64" => 11,
        "G15"|"G16" => 12, "G50"|"G51" => 13, "G68"|"G69" => 14,
        "M0"|"M1"|"M2"|"M30" => 21, "M3"|"M4"|"M5" => 22, "M6" => 23,
        "M48"|"M49" => 24, _ => return None,
    })
}
pub(crate) fn interpreted(c: &str) -> bool {
    matches!(c,"G0"|"G1"|"G2"|"G3"|"G4"|"G17"|"G18"|"G19"|"G20"|"G21"|"G40"|"G49"|"G53"|"G54"|"G55"|"G56"|"G57"|"G58"|"G59"|"G61"|"G61.1"|"G64"|"G70"|"G71"|"G80"|"G90"|"G91"|"G90.1"|"G91.1"|"G93"|"G94"|"G95"|"G98"|"G99"|"M0"|"M1"|"M2"|"M3"|"M4"|"M5"|"M6"|"M7"|"M8"|"M9"|"M30"|"M48"|"M49")
}
