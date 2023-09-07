@tool
extends VoxelChunk

@export
var is_generated: bool = false
	
func _process(delta):
	if not is_generated:
		is_generated = true
		self.generate()
		load_mesh()

func load_mesh():
	var mesh_data = []
	mesh_data.clear()
	mesh_data.resize(Mesh.ARRAY_MAX)
	mesh_data[Mesh.ARRAY_VERTEX] = mesh_positions
	mesh_data[Mesh.ARRAY_INDEX] = mesh_indices
	#mesh_data[Mesh.ARRAY_NORMAL] = mesh_normals
	#mesh_data[Mesh.ARRAY_TANGENT] = mesh_tangents

	var amesh = ArrayMesh.new()
	amesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, mesh_data)
	
	print("[SurfaceTool] Generating normals and tangents...")
	var st: SurfaceTool = SurfaceTool.new()
	st.create_from(amesh, 0)
	st.generate_normals()
	st.generate_tangents()
	print("[SurfaceTool] Done!")
	
	mesh = st.commit()
