class AnimationManager {
    constructor() {
        this.animations = new Map();
        this.isAnimating = false;
    }

    // Create a new animation for two-phase vertical-then-direct movement
    createMoveAnimation(mesh, startPos, endPos, duration = 500, liftHeight = null) {
        // Calculate appropriate lift height if not provided
        if (liftHeight === null) {
            const heightDiff = endPos.z - startPos.z;
            // Remember: negative Z is "up" in our stack system
            liftHeight = heightDiff < 0 ? Math.abs(heightDiff) + 0.5 : 0;
        }

        const animation = {
            mesh: mesh,
            startPos: startPos.clone(),
            endPos: endPos.clone(),
            liftHeight: liftHeight,
            startTime: Date.now(),
            duration: duration,
            phase: liftHeight > 0 ? 'lift' : 'move',
            liftDuration: duration * 0.4, // 40% of time for lift
            moveDuration: duration * 0.6,  // 60% of time for movement
            onComplete: null,
            phaseStartTime: Date.now()
        };
        
        this.animations.set(mesh.uuid, animation);
        
        if (!this.isAnimating) {
            this.isAnimating = true;
            this.animate();
        }

        return new Promise((resolve) => {
            animation.onComplete = resolve;
        });
    }

    // Calculate position for two-phase movement
    calculatePosition(animation, currentTime) {
        const elapsed = currentTime - animation.phaseStartTime;
        
        // If no lift needed, go straight to move phase
        if (animation.liftHeight === 0) {
            animation.phase = 'move';
        }

        if (animation.phase === 'lift') {
            // First phase: Vertical lift only (in negative Z direction)
            const liftProgress = Math.min(elapsed / animation.liftDuration, 1);
            const easedProgress = this.easeInOutCubic(liftProgress);
            
            // Create position with only vertical movement in Z axis
            const pos = animation.startPos.clone();
            // Subtract from Z because we're moving upward (negative Z)
            pos.z -= animation.liftHeight * easedProgress;

            // When lift is complete, switch to move phase
            if (liftProgress >= 1) {
                animation.phase = 'move';
                animation.phaseStartTime = currentTime; // Reset timer for move phase
                // Ensure we're at exactly the right height before moving
                pos.z = animation.startPos.z - animation.liftHeight;
                return pos;
            }
            
            return pos;
        } else {
            // Second phase: Direct movement to target
            const moveProgress = Math.min(elapsed / animation.moveDuration, 1);
            const easedProgress = this.easeInOutCubic(moveProgress);
            
            // Start from the fully lifted position
            const liftedStart = animation.startPos.clone();
            liftedStart.z = animation.startPos.z - animation.liftHeight;
            
            // Keep X and Y from start position until we start moving
            const pos = new THREE.Vector3();
            
            // Interpolate X and Y position
            pos.x = liftedStart.x + (animation.endPos.x - liftedStart.x) * easedProgress;
            pos.y = liftedStart.y + (animation.endPos.y - liftedStart.y) * easedProgress;
            
            // Interpolate Z position from lifted height to final height
            pos.z = liftedStart.z + (animation.endPos.z - liftedStart.z) * easedProgress;
            
            return pos;
        }
    }

    // Animate all active animations
    animate() {
        if (this.animations.size === 0) {
            this.isAnimating = false;
            return;
        }

        const currentTime = Date.now();
        const animationsToRemove = [];

        this.animations.forEach((animation, uuid) => {
            const elapsed = currentTime - animation.phaseStartTime;
            
            // Calculate new position
            const newPos = this.calculatePosition(animation, currentTime);
            animation.mesh.position.copy(newPos);

            // Check if animation is complete
            if (animation.phase === 'move' && elapsed >= animation.moveDuration) {
                // Ensure we end exactly at the target position
                animation.mesh.position.copy(animation.endPos);
                animationsToRemove.push(uuid);
                if (animation.onComplete) {
                    animation.onComplete();
                }
            }
        });

        // Remove completed animations
        animationsToRemove.forEach(uuid => {
            this.animations.delete(uuid);
        });

        // Continue animation loop
        if (this.animations.size > 0) {
            requestAnimationFrame(() => this.animate());
        } else {
            this.isAnimating = false;
        }
    }

    // Cubic easing function for smooth movement
    easeInOutCubic(t) {
        return t < 0.5
            ? 4 * t * t * t
            : 1 - Math.pow(-2 * t + 2, 3) / 2;
    }

    // Check if a mesh is currently being animated
    isAnimating(mesh) {
        return this.animations.has(mesh.uuid);
    }

    // Cancel any ongoing animation for a mesh
    cancelAnimation(mesh) {
        if (this.animations.has(mesh.uuid)) {
            const animation = this.animations.get(mesh.uuid);
            if (animation.onComplete) {
                animation.onComplete();
            }
            this.animations.delete(mesh.uuid);
        }
    }
}

// Remove old GlowEffect class and add new GlowEffect class for scoring animations using a separate glow mesh
class GlowEffect {
    constructor(originalMesh, color, duration = 1000) {
        this.originalMesh = originalMesh;
        this.duration = duration;
        this.startTime = null;
        this.isActive = false;
        
        // Determine glow color
        this.glowColor = (typeof color === 'number') ? new THREE.Color(color) : color;
        
        // Create the outer glow sphere with larger initial size and more segments
        this.sphereGeometry = new THREE.SphereGeometry(0.6, 32, 32);
        this.sphereMaterial = new THREE.ShaderMaterial({
            uniforms: {
                color: { value: this.glowColor },
                opacity: { value: 1.0 },
                power: { value: 2.0 },
                scale: { value: 1.0 }  // Add scale uniform
            },
            vertexShader: `
                varying vec3 vNormal;
                varying vec3 vViewPosition;
                uniform float scale;
                varying float vScale;
                
                void main() {
                    vScale = scale;
                    vNormal = normalize(normalMatrix * normal);
                    vec4 mvPosition = modelViewMatrix * vec4(position, 1.0);
                    vViewPosition = -mvPosition.xyz;
                    gl_Position = projectionMatrix * mvPosition;
                }
            `,
            fragmentShader: `
                uniform vec3 color;
                uniform float opacity;
                uniform float power;
                varying float vScale;
                
                varying vec3 vNormal;
                varying vec3 vViewPosition;
                
                void main() {
                    vec3 normal = normalize(vNormal);
                    vec3 viewDir = normalize(vViewPosition);
                    float fresnel = pow(1.0 - abs(dot(normal, viewDir)), power);
                    // Make the distance fade much more aggressive
                    float distanceFade = 1.0 / (vScale * vScale * vScale * 2.0);
                    gl_FragColor = vec4(color, opacity * fresnel * distanceFade);
                }
            `,
            transparent: true,
            blending: THREE.AdditiveBlending,
            depthWrite: false,
            side: THREE.DoubleSide
        });
        this.pulseMesh = new THREE.Mesh(this.sphereGeometry, this.sphereMaterial);
        this.pulseMesh.renderOrder = 999; // Ensure it renders on top
        
        // Create the shrinking glow mesh (clone of original)
        this.glowMesh = originalMesh.clone();
        this.glowMaterial = new THREE.MeshBasicMaterial({
            color: this.glowColor,
            transparent: true,
            opacity: 1.0,
            blending: THREE.AdditiveBlending,
            depthWrite: false
        });
        this.glowMesh.material = this.glowMaterial;
        this.glowMesh.renderOrder = 998;
    }
    
    start() {
        this.startTime = Date.now();
        this.isActive = true;
        
        // Hide the original mesh
        this.originalMesh.visible = false;
        
        // Get the original mesh's world position and quaternion
        const worldPos = new THREE.Vector3();
        const worldQuat = new THREE.Quaternion();
        this.originalMesh.getWorldPosition(worldPos);
        this.originalMesh.getWorldQuaternion(worldQuat);
        
        // If the original mesh has a parent, convert the world transforms into the parent's local space
        if (this.originalMesh.parent) {
            // For position:
            const localPos = worldPos.clone();
            this.originalMesh.parent.worldToLocal(localPos);
            this.pulseMesh.position.copy(localPos);
            this.glowMesh.position.copy(localPos);
            
            // For rotation: get parent's world quaternion, invert it, and multiply with worldQuat
            const parentQuat = new THREE.Quaternion();
            this.originalMesh.parent.getWorldQuaternion(parentQuat);
            parentQuat.invert();
            const localQuat = worldQuat.clone().premultiply(parentQuat);
            this.pulseMesh.quaternion.copy(localQuat);
            this.glowMesh.quaternion.copy(localQuat);
        } else {
            this.pulseMesh.position.copy(worldPos);
            this.glowMesh.position.copy(worldPos);
            this.pulseMesh.quaternion.copy(worldQuat);
            this.glowMesh.quaternion.copy(worldQuat);
        }
        
        // Add both meshes to the parent
        if (this.originalMesh.parent) {
            this.originalMesh.parent.add(this.pulseMesh);
            this.originalMesh.parent.add(this.glowMesh);
        }
    }
    
    update() {
        if (!this.isActive) return false;
        
        const elapsed = Date.now() - this.startTime;
        const progress = Math.min(elapsed / this.duration, 1);
        
        if (progress >= 1) {
            // Ensure we're completely invisible before stopping
            this.pulseMesh.scale.set(0, 0, 0);
            this.glowMesh.scale.set(0, 0, 0);
            this.sphereMaterial.uniforms.opacity.value = 0;
            this.glowMaterial.opacity = 0;
            this.stop();
            return false;
        }
        
        // Animate pulse sphere: expand outward with easing
        const pulseScale = 1 + (4 * Math.pow(progress, 0.5));
        this.pulseMesh.scale.set(pulseScale, pulseScale, pulseScale);
        this.sphereMaterial.uniforms.scale.value = pulseScale;  // Update scale uniform
        this.sphereMaterial.uniforms.opacity.value = Math.max(0, 1 - (progress * 0.8)); // Reduced base fade
        this.sphereMaterial.uniforms.power.value = 1 + progress * 2;
        
        // Animate original mesh: smoother shrink with complete fade before end
        const shrinkProgress = Math.min(progress * 1.2, 1);
        const shrinkScale = Math.max(0, 1 - shrinkProgress);
        this.glowMesh.scale.set(shrinkScale, shrinkScale, shrinkScale);
        this.glowMaterial.opacity = Math.max(0, 1 - (progress * 1.2));
        
        return true;
    }
    
    stop() {
        // Never show the original mesh - let the parent code handle cleanup
        this.originalMesh.visible = false;
        
        // Remove both meshes from the parent
        if (this.pulseMesh.parent) {
            this.pulseMesh.parent.remove(this.pulseMesh);
        }
        if (this.glowMesh.parent) {
            this.glowMesh.parent.remove(this.glowMesh);
        }
        
        // Clean up only our own materials and geometries
        this.sphereGeometry.dispose();
        this.sphereMaterial.dispose();
        this.glowMaterial.dispose();
        
        this.isActive = false;
    }
}

// Add to window scope
window.GlowEffect = GlowEffect;

// Export the AnimationManager
window.AnimationManager = AnimationManager;