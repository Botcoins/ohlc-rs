use model::*;

#[derive(Clone, Debug)]
pub struct TestText;

impl RendererExtension for TestText {
    fn apply(&self, buffer: &mut ChartBuffer, _data: &[Candle]) {
        buffer.text((0, 0), "DANKMEME", 0xFFFF00FF);
        buffer.text((0, 60), "DANKMEME", 0xFFFF007F);
    }

    fn lore_colour(&self) -> Option<u32> {
        None
    }

    fn name(&self) -> String {
        "TEST_Text()".to_string()
    }
}

impl PartialEq for TestText {
    fn eq(&self, _: &TestText) -> bool {
        true
    }
}