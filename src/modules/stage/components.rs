use bevy::prelude::*;

#[derive(Component)]
pub(crate) struct ScreenMarker;

#[derive(Resource)]
pub(crate) struct AssetHandles {
    pub(crate) screens: Vec<Handle<Image>>,
    /// `CGDirectDisplayID` for each screen, in the same order as `screens`.
    #[expect(dead_code, reason = "reserved for future display routing")]
    pub(crate) display_ids: Vec<u32>,
}
