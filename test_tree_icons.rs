// Quick standalone test to check tree icon rendering
use std::collections::HashMap;
use tgui::prelude::*;

#[derive(Clone)]
struct TestItem {
    name: String,
}

struct TestVm {
    expanded: State<Vec<WidgetKey>>,
}

impl ViewModel for TestVm {
    fn new(ctx: &ViewModelContext) -> Self {
        Self {
            expanded: ctx.state(vec![WidgetKey::from("root")]),
        }
    }

    fn view(&self) -> Element<Self> {
        let nodes = vec![
            TreeNode::keyed(
                "root",
                TestItem {
                    name: "Root".to_string(),
                },
            )
            .child(TreeNode::keyed(
                "child",
                TestItem {
                    name: "Child".to_string(),
                },
            )),
        ];

        Tree::new(nodes, |ctx| {
            Text::new(ctx.item.name.clone()).into()
        })
        .expanded_keys(self.expanded.signal())
        .checkable(true)
        .width(dp(400.0))
        .height(dp(300.0))
        .into()
    }
}

fn main() {
    println!("Tree icon rendering test - check if icons are visible");

    Application::new()
        .title("Tree Icon Test")
        .window_size(dp(500.0), dp(400.0))
        .with_view_model(TestVm::new)
        .root_view(TestVm::view)
        .run()
        .expect("Failed to run application");
}
