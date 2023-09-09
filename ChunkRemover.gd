extends Area3D


# Called when the node enters the scene tree for the first time.
func _ready():
	body_exited.connect(remove_chunk)

# Called every frame. 'delta' is the elapsed time since the previous frame.
func _process(delta):
	pass

func remove_chunk(body):
	if is_instance_of(body.owner, VoxelChunk):
		body.queue_free()
