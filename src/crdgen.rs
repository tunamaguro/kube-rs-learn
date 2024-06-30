use controller::markdown;
use kube::CustomResourceExt;
use markdown::MarkdownView;

fn main() {
    print!("{}", serde_yaml::to_string(&MarkdownView::crd()).unwrap())
}
