extends CharacterBody2D
class_name Player
## A simple top-down player controller.

signal health_changed(old_value: int, new_value: int)
signal died

enum State { IDLE, RUNNING, HURT }

const MAX_HEALTH := 100
const SPEED: float = 240.0

@export var acceleration := 1200.0
@export_range(0.0, 1.0) var friction: float = 0.15
@onready var sprite: Sprite2D = $Sprite2D

var state := State.IDLE
var health: int = MAX_HEALTH:
    set(value):
        var old := health
        health = clampi(value, 0, MAX_HEALTH)
        health_changed.emit(old, health)
        if health == 0:
            died.emit()


func _physics_process(delta: float) -> void:
    var input := Input.get_vector("left", "right", "up", "down")
    if input != Vector2.ZERO:
        velocity = velocity.move_toward(input * SPEED, acceleration * delta)
        state = State.RUNNING
    else:
        velocity = velocity.lerp(Vector2.ZERO, friction)
        state = State.IDLE
    move_and_slide()
    sprite.flip_h = velocity.x < 0


func take_damage(amount: int, source: Node = null) -> bool:
    # Returns true when the hit was fatal.
    if state == State.HURT or amount <= 0:
        return false
    health -= amount
    print("Hit by %s for %d\n" % [source.name if source else "unknown", amount])
    return health <= 0
