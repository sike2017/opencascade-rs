use opencascade::{
    primitives
};

fn dfs_traverse(label: &mut primitives::TdfLabel, shape_tool: &mut primitives::XCAFDocShapeTool) {
    for index in 0..label.get_child_number() {
        let mut label = label.get_child(index);
        println!("Child Label Index: {}: Name: {}", index, label.get_name());
        dfs_traverse(&mut label, shape_tool);
    }
}

#[test]
fn it_can_read_caf_step() {
    let step_path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/assets/00000050_80d90bfdd2e74e709956122a_step_000.step");
    println!("Reading STEP file from path: {}", step_path);
    let (top_labels, mut shape_tool) = primitives::TdfLabel::read_caf_step(step_path).expect("Failed to read STEP file");
    for mut label in top_labels {
        let label_shape = label.get_shape(&mut shape_tool);
        if label_shape.is_none() {
            println!("top label: Name: {}, No shape associated", label.get_name());
            continue;
        }
        let shape = label_shape.unwrap();
        let label_mesh = shape.mesh().expect("not a valid triangulation");
        println!("top label: Name: {}, Vertices: {}, Indices: {}", label.get_name(), label_mesh.vertices.len(), label_mesh.indices.len());
        dfs_traverse(&mut label, &mut shape_tool);
    }
}
