extends VoxelChunk

var done = false

# Called when the node enters the scene tree for the first time.
func _ready():
	pass # Replace with function body.


# Called every frame. 'delta' is the elapsed time since the previous frame.
func _process(delta):
	if !done and mesh != null:
		done = true
		call_deferred("create_trimesh_collision")
