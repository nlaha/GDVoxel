#@tool
extends VoxelChunk
	
func _ready():
	#call_deferred("load_mesh")
	pass
	
func _process(delta):
	pass
#	for i in range(0, mesh_normals.size()):
#		DebugDraw.draw_line_3d(mesh_positions[i], mesh_positions[i] + mesh_normals[i], Color.RED)
#	if not is_generated:
#		is_generated = true
#		self.generate()
#		load_mesh()

func load_mesh():
	
	print("[GDVoxel - Loader] Loading voxel mesh into godot...")
	var mesh_data = []
	mesh_data.resize(Mesh.ARRAY_MAX)
	mesh_data[Mesh.ARRAY_VERTEX] = mesh_positions
	mesh_data[Mesh.ARRAY_INDEX] = mesh_indices
	mesh_data[Mesh.ARRAY_NORMAL] = mesh_normals

	mesh = ArrayMesh.new()
	mesh.add_surface_from_arrays(Mesh.PRIMITIVE_TRIANGLES, mesh_data)
	print("[GDVoxel - Loader] Done!")

	#print(mesh_data[Mesh.ARRAY_VERTEX])
