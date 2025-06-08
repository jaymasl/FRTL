"use strict";

// *** Define the WEBGL object directly ***
const WEBGL = {
	isWebGLAvailable: function () {
		try {
			const canvas = document.createElement( 'canvas' );
			return !! ( window.WebGLRenderingContext && ( canvas.getContext( 'webgl' ) || canvas.getContext( 'experimental-webgl' ) ) );
		} catch ( e ) {
			return false;
		}
	},

	isWebGL2Available: function () {
		try {
			const canvas = document.createElement( 'canvas' );
			return !! ( window.WebGL2RenderingContext && canvas.getContext( 'webgl2' ) );
		} catch ( e ) {
			return false;
		}
	},

	getWebGLErrorMessage: function () {
		return this.getErrorMessage( 1 );
	},

	getWebGL2ErrorMessage: function () {
		return this.getErrorMessage( 2 );
	},

	getErrorMessage: function ( version ) {
		const names = {
			1: 'WebGL',
			2: 'WebGL 2'
		};

		const contexts = {
			1: window.WebGLRenderingContext,
			2: window.WebGL2RenderingContext
		};

		let message = 'Your $0 does not seem to support <a href="http://khronos.org/webgl/wiki/Getting_a_WebGL_Implementation" style="color:#000">$1</a>';

		const element = document.createElement( 'div' );
			element.id = 'webgl-error-message';
			element.style.fontFamily = 'monospace';
			element.style.fontSize = '13px';
			element.style.fontWeight = 'normal';
			element.style.textAlign = 'center';
			element.style.background = '#fff';
			element.style.color = '#000';
			element.style.padding = '1.5em';
			element.style.width = '400px';
			element.style.margin = '5em auto 0';

		if ( contexts[ version ] ) {
			message = message.replace( '$0', 'graphics card' );
		} else {
			message = message.replace( '$0', 'browser' );
		}

		message = message.replace( '$1', names[ version ] );

			element.innerHTML = message;

		return element;
	}
};
// *** End of WEBGL definition ***

// Game configuration constants
const GameConfig = {
    BOARD: {
        HEX_RADIUS: 0.35,
        GAP_FACTOR: 1.1,
        TILE_HEIGHT: 0.1,
        PLACEMENT_THRESHOLD: 0.7
    },
    CAMERA: {
        INITIAL_DISTANCE: 7,
        INITIAL_HEIGHT: 7,
        FOV: 45,
        NEAR: 0.1,
        FAR: 1000,
        ROTATION_SPEED: 0.15
    },
    RENDERER: {
        WIDTH: 720,
        HEIGHT: 1280,
        BACKGROUND_COLOR: 0x000000
    },
    DASHBOARD: {
        WIDTH: 1.5,
        HEIGHT: 1.5,
        COLOR: 0x222222,
        OPACITY: 0.8,
        POSITION: { x: 0, y: -1.5, z: -3 }
    },
    DRAGGABLE: {
        SCALE: {
            NORMAL: 1.5,
            MINIMIZED: 0.3
        },
        POSITION: { x: 0, y: -0.85, z: -2.5 },
        ROTATION: { x: 2, y: 0, z: 0 }
    }
};

// Function to initialize game session communication with parent window
// Returns true if session is established, false otherwise
async function initGameSession() {
    console.log('Initializing game session communication...');
    window.gameSession = null; // Ensure it starts as null
    window.gameSessionSignature = null;

    // Restore from sessionStorage first, if available
    try {
        const backup_session = sessionStorage.getItem('hexort_game_session');
        const backup_signature = sessionStorage.getItem('hexort_game_signature');
        if (backup_session && backup_signature) {
            console.log('Session immediately restored from sessionStorage');
            window.gameSession = backup_session;
            window.gameSessionSignature = backup_signature;
            // No need to contact parent if restored locally
             // Set up listener anyway in case parent sends unsolicited updates? Less critical.
             window.addEventListener('message', handleSessionMessage);
            return true;
        }
    } catch (e) {
        console.warn('Could not access sessionStorage or restore session:', e);
    }

    return new Promise((resolve) => {
        let sessionReceived = false;
        let requestIntervalId = null;
        let overallTimeoutId = null;
        const MAX_WAIT_TIME = 15000; // 15 seconds total wait time
        const REQUEST_INTERVAL = 500; // Request every 500ms

        // Shared message handler function
        function handleSessionMessage(event) {
            // Basic origin check (replace '*' with parent origin in production)
            // if (event.origin !== 'expected_parent_origin') return;

            if (event.data && event.data.type === 'session_info') {
                console.log('Received session_info from parent:', event.data);
                if (event.data.session_id && typeof event.data.session_id === 'string' && event.data.session_id.includes(':')) {
                    window.gameSession = event.data.session_id; // Expecting combined ID:HMAC
                    window.gameSessionSignature = event.data.session_signature || ''; // Store signature if provided

                    // Store in sessionStorage for future loads
                    try {
                        sessionStorage.setItem('hexort_game_session', window.gameSession);
                        sessionStorage.setItem('hexort_game_signature', window.gameSessionSignature);
                        console.log('Session info backed up to sessionStorage');
                    } catch (e) {
                        console.warn('Could not store session in sessionStorage:', e);
                    }

                    sessionReceived = true;
                    cleanupTimers(); // Stop requesting and timeout
                    resolve(true); // Resolve the promise indicating success

                    // Optional: Update UI if needed now that session is known
                    // if (window.updateUIForSession) window.updateUIForSession();

                } else {
                     console.warn('Received session_info, but session_id format is invalid or missing.');
                     // Keep trying? Or treat as failure? For now, keep trying.
                }
            }
        }

        function cleanupTimers() {
            if (requestIntervalId) {
                clearInterval(requestIntervalId);
                requestIntervalId = null;
                console.log("Cleared session request interval.");
            }
            if (overallTimeoutId) {
                clearTimeout(overallTimeoutId);
                overallTimeoutId = null;
            }
             // Keep listener active? Or remove? Let's remove it if we resolve/reject.
             window.removeEventListener('message', handleSessionMessage);
             console.log("Removed session message listener.");
        }

        // Add the message listener
        window.addEventListener('message', handleSessionMessage);
        console.log("Added session message listener.");

        // Function to send the request
        function requestSession() {
            if (sessionReceived) return; // Stop if already received
            console.log('Posting REQUEST_SESSION_INFO to parent.');
            try {
                window.parent.postMessage({
                    type: 'REQUEST_SESSION_INFO',
                    timestamp: Date.now()
                }, '*'); // Use specific origin in production
            } catch (e) {
                 console.error("Error posting message to parent:", e);
                 // Potentially stop trying if postMessage fails consistently
            }
        }

        // Start the request interval
        requestIntervalId = setInterval(requestSession, REQUEST_INTERVAL);
        console.log(`Started session request interval (${REQUEST_INTERVAL}ms).`);

        // Set an overall timeout
        overallTimeoutId = setTimeout(() => {
            if (!sessionReceived) {
                console.error(`Timeout (${MAX_WAIT_TIME}ms) waiting for session_info from parent. Game may not function correctly.`);
                cleanupTimers();
                resolve(false); // Resolve indicating failure after timeout
            }
        }, MAX_WAIT_TIME);

        // Make the initial request immediately
        requestSession();
    });
}

// Game state management class
class GameState {
    constructor() {
        this.isDragging = false;
        this.isMovingPiece = false;
        this.isRotating = false;
        this.isGameOver = false;
        this.draggableHexagons = [null, null, null]; // Array of three active draggables
        this.selectedDraggableIndex = -1; // Track which stack is being dragged
        this.draggableStartPositions = [
            new THREE.Vector3(),
            new THREE.Vector3(),
            new THREE.Vector3()
        ];
        this.previousMousePosition = { x: 0, y: 0 };
    }

    setDraggableHexagon(hexagon, index) {
        this.draggableHexagons[index] = hexagon;
    }

    updateDraggableStartPosition(position, index) {
        this.draggableStartPositions[index].copy(position);
    }

    resetState() {
        if (!this.isGameOver) {
            this.isDragging = false;
            this.isMovingPiece = false;
            this.selectedDraggableIndex = -1;
        }
    }
}

// Camera controller class
class CameraController {
    constructor(camera, initialDistance = 7, initialHeight = 7) {
        this.camera = camera;
        this.cameraDistance = initialDistance;
        this.cameraHeight = initialHeight;
        this.cameraAngle = 2.62;
        this.targetCameraAngle = 0;
        this.isRotating = false;
        this.rotationSpeed = 0.15;
    }

    updatePosition() {
        this.camera.position.x = this.cameraDistance * Math.cos(this.cameraAngle);
        this.camera.position.y = this.cameraHeight;
        this.camera.position.z = this.cameraDistance * Math.sin(this.cameraAngle);
        this.camera.lookAt(0, 0, 0);
        this.camera.up.set(0, 1, 0);
    }

    handleRotation() {
        if (this.isRotating) {
            const angleDiff = this.targetCameraAngle - this.cameraAngle;
            if (Math.abs(angleDiff) < 0.01) {
                this.cameraAngle = this.targetCameraAngle;
                this.isRotating = false;
            } else {
                this.cameraAngle += angleDiff * this.rotationSpeed;
                this.updatePosition();
            }
        }
    }

    startRotation(angle) {
        this.targetCameraAngle = Math.round(angle / (Math.PI / 6)) * (Math.PI / 6);
        this.isRotating = true;
    }

    updateAngle(deltaX) {
        this.cameraAngle += deltaX * 0.01;
        this.updatePosition();
    }
}

// Scene management class
class SceneManager {
    constructor() {
        this.scene = new THREE.Scene();
        this.setupLights();
        console.log("SceneManager initialized.");
    }

    setupLights() {
        const ambientLight = new THREE.AmbientLight(0xffffff, 0.6);
        this.scene.add(ambientLight);

        // Primary directional light
        const directionalLight = new THREE.DirectionalLight(0xffffff, 0.6);
        directionalLight.position.set(2, 4, 2);
        this.scene.add(directionalLight);

        // Add opposite directional light
        const oppositeLight = new THREE.DirectionalLight(0xffffff, 0.4);
        oppositeLight.position.set(-2, 4, -2);
        this.scene.add(oppositeLight);
    }

    add(object) {
        this.scene.add(object);
    }

    remove(object) {
        this.scene.remove(object);
    }

    getScene() {
        return this.scene;
    }
}

// Add UI Scene class at the top level
class UIScene {
    constructor(width, height) {
        this.width = width;
        this.height = height;
        this.scene = new THREE.Scene();
        this.scene.name = "UI Scene";
        
        // Recreate the camera definition
        this.camera = new THREE.OrthographicCamera(
            width / -2, width / 2,
            height / 2, height / -2,
            1, 10
        );
        this.camera.position.z = 5;
        
        this.scoreDisplay = null;
        this.raycaster = new THREE.Raycaster();
        
        this.createScoreDisplay();

        console.log("UIScene initialized with score display.");
    }

    createScoreDisplay() {
        const canvas = document.createElement('canvas');
        const context = canvas.getContext('2d');
        canvas.width = 256;
        canvas.height = 64;
        this.scoreTexture = new THREE.CanvasTexture(canvas);
        this.scoreTexture.minFilter = THREE.LinearFilter;
        this.scoreContext = context;

        const scoreGeometry = new THREE.PlaneGeometry(200, 50);
        const scoreMaterial = new THREE.MeshBasicMaterial({
            map: this.scoreTexture,
            transparent: true,
            depthTest: false,
            depthWrite: false
        });
        this.scorePlane = new THREE.Mesh(scoreGeometry, scoreMaterial);
        this.scorePlane.position.set(0, this.height / 2 - 50, 2);
        this.scorePlane.renderOrder = 9999;
        this.scene.add(this.scorePlane);
        this.updateScore(0);
    }

    updateScore(score) {
        this.scoreContext.clearRect(0, 0, 256, 64);
        this.scoreContext.fillStyle = 'rgba(0, 0, 0, 0.8)';
        this.scoreContext.fillRect(0, 0, 256, 64);
        this.scoreContext.fillStyle = 'white';
        this.scoreContext.font = 'bold 32px Arial';
        this.scoreContext.textAlign = 'center';
        this.scoreContext.textBaseline = 'middle';
        this.scoreContext.fillText(`Score: ${score}`, 128, 32);
        this.scoreTexture.needsUpdate = true;
    }
}

// Draggable manager class
class DraggableManager {
    constructor(camera, gameState, uiScene) {
        this.camera = camera;
        this.gameState = gameState;
        this.uiScene = uiScene;
        this.draggablePositions = [
            { x: -0.3, y: -0.85, z: -2.5 }, // Left
            { x: 0, y: -0.85, z: -2.5 },    // Center
            { x: 0.3, y: -0.85, z: -2.5 }   // Right
        ];
        this.setupDashboard();
        this.initDashboardOptions();
        this.initializeDraggables();
        console.log("DraggableManager initialized.");
    }

    initializeDraggables() {
        // Create initial draggable hexagon stacks
        for (let i = 0; i < 3; i++) {
            const draggableHexagon = this.createNewDraggable();
            this.gameState.setDraggableHexagon(draggableHexagon, i);
            this.camera.add(draggableHexagon.getMesh());
            
            const pos = this.draggablePositions[i];
            draggableHexagon.getMesh().position.set(pos.x, pos.y, pos.z);
            this.gameState.updateDraggableStartPosition(draggableHexagon.getMesh().position, i);
            
            draggableHexagon.getMesh().rotation.set(
                GameConfig.DRAGGABLE.ROTATION.x,
                GameConfig.DRAGGABLE.ROTATION.y,
                GameConfig.DRAGGABLE.ROTATION.z
            );
            draggableHexagon.getMesh().scale.set(
                GameConfig.DRAGGABLE.SCALE.MINIMIZED,
                GameConfig.DRAGGABLE.SCALE.MINIMIZED,
                GameConfig.DRAGGABLE.SCALE.MINIMIZED
            );
            draggableHexagon.getMesh().renderOrder = 10;
        }
    }

    setupDashboard() {
        const geometry = new THREE.PlaneGeometry(GameConfig.DASHBOARD.WIDTH, GameConfig.DASHBOARD.HEIGHT);
        const material = new THREE.MeshBasicMaterial({ 
            color: GameConfig.DASHBOARD.COLOR, 
            opacity: GameConfig.DASHBOARD.OPACITY, 
            transparent: true 
        });
        const dashboard = new THREE.Mesh(geometry, material);
        const pos = GameConfig.DASHBOARD.POSITION;
        dashboard.position.set(pos.x, pos.y, pos.z);
        dashboard.rotation.set(0, 0, 0);
        dashboard.renderOrder = 10;
        this.dashboard = dashboard;  // Store reference to dashboard
        this.dashboardMaterial = material;  // Store reference to material
        this.camera.add(dashboard);
    }

    initDashboardOptions() {
        // Create a container for dashboard options and add it to the camera
        this.dashboardOptions = [];
        this.dashboardContainer = new THREE.Group();
        this.dashboardContainer.renderOrder = 22;  // Ensure container renders on top
        // Position the container to align with the dashboard
        this.dashboardContainer.position.copy(GameConfig.DASHBOARD.POSITION);
        // Move it slightly forward to ensure it's in front of the dashboard
        this.dashboardContainer.position.z += 0.1;
        this.camera.add(this.dashboardContainer);

        // Create three dashboard option stacks
        for (let i = 0; i < 3; i++) {
            const option = this.createDashboardOption(i);
            this.dashboardOptions[i] = option;
            this.dashboardContainer.add(option.getMesh());
            // Position options horizontally with wider spacing
            const spacing = GameConfig.DASHBOARD.WIDTH / 4;  // Divide dashboard width into 4 segments
            option.getMesh().position.set((i - 1) * spacing, 0, 0);
            // Rotate options to face forward
            option.getMesh().rotation.set(0, 0, 0);
        }
    }

    createDashboardOption(index) {
        const newOption = this.createNewDraggable();
        // Mark its mesh as a dashboard option and store the option index
        newOption.getMesh().userData.isDashboardOption = true;
        newOption.getMesh().userData.optionIndex = index;
        // Ensure dashboard options render on top
        newOption.getMesh().renderOrder = 21;
        // Scale down the option for dashboard display
        newOption.getMesh().scale.set(0.3, 0.3, 0.3);
        return newOption;
    }

    selectDashboardOption(index) {
        const selectedOption = this.dashboardOptions[index];
        // Remove the selected option from the dashboard container
        this.dashboardContainer.remove(selectedOption.getMesh());
        // Replace the option with a new random stack to keep three options always available
        this.dashboardOptions[index] = this.createDashboardOption(index);
        // Position the new option in the same spot as the old one
        const spacing = GameConfig.DASHBOARD.WIDTH / 4;
        this.dashboardOptions[index].getMesh().position.set((index - 1) * spacing, 0, 0);
        this.dashboardContainer.add(this.dashboardOptions[index].getMesh());
        // Return the selected (now removed) option for use
        return selectedOption;
    }

    updateDraggablePosition() {
        for (let i = 0; i < 3; i++) {
            if (!this.gameState.isMovingPiece || this.gameState.selectedDraggableIndex !== i) {
                const draggable = this.gameState.draggableHexagons[i];
                if (draggable) {
                    const draggableMesh = draggable.getMesh();
                    if (draggableMesh.parent !== this.camera) {
                        if (draggableMesh.parent) {
                            draggableMesh.parent.remove(draggableMesh);
                        }
                        this.camera.add(draggableMesh);
                    }
                    
                    const pos = this.draggablePositions[i];
                    const rot = GameConfig.DRAGGABLE.ROTATION;
                    draggableMesh.position.set(pos.x, pos.y, pos.z);
                    this.gameState.updateDraggableStartPosition(draggableMesh.position, i);
                    
                    draggableMesh.rotation.set(rot.x, rot.y, rot.z);
                    draggableMesh.scale.set(
                        GameConfig.DRAGGABLE.SCALE.MINIMIZED,
                        GameConfig.DRAGGABLE.SCALE.MINIMIZED,
                        GameConfig.DRAGGABLE.SCALE.MINIMIZED
                    );
                    
                    draggableMesh.renderOrder = 10;
                }
            }
        }
    }

    createNewDraggable() {
        const newDraggable = new DraggableHexagonStack(
            GameConfig.BOARD.HEX_RADIUS,
            Math.floor(Math.random() * 5) + 1
        );
        return newDraggable;
    }

    updateScoreDisplay(score) {
        if (this.uiScene) {
            this.uiScene.updateScore(score);
        }
    }

    // Add method to show sorting state
    setSortingState(isSorting) {
        if (this.dashboardMaterial) {
            if (isSorting) {
                // Change to darker red when sorting
                this.dashboardMaterial.color.setHex(0x330000);
                this.dashboardMaterial.opacity = 0.3;
            } else {
                // Reset to original color
                this.dashboardMaterial.color.setHex(GameConfig.DASHBOARD.COLOR);
                this.dashboardMaterial.opacity = GameConfig.DASHBOARD.OPACITY;
            }
        }
    }
}

async function initAudio() {
    // Initialize audio manager
    window.audioManager = new AudioManager();
    
    // Add click handler to resume audio context
    document.addEventListener('click', () => {
        if (window.audioManager) {
            window.audioManager.resumeAudio();
        }
    }, { once: true });  // Only need to do this once

    // Start audio initialization but don't wait for it
    window.audioManager.init().then(() => {
        console.log('Audio system initialized');
    }).catch(error => {
        console.error('Error initializing audio:', error);
    });
}

async function initGame(renderer) {
    console.log("initGame started.");
    // Strict validation of game state at initialization
    console.log('Performing strict game state validation...');
    
    // Force new ColorManager instance and verify initial colors
    ColorManager.instance = null;
    const colorManager = new ColorManager();
    const availableColors = Array.from(colorManager.availableColors);
    const expectedInitialColors = ['RED', 'GREEN', 'BLUE'];
    
    // Verify we only have initial colors
    const hasOnlyInitialColors = availableColors.length === expectedInitialColors.length &&
        expectedInitialColors.every(color => availableColors.includes(color));
    
    console.log('Initial color state:', {
        availableColors,
        expectedInitialColors,
        isCorrect: hasOnlyInitialColors
    });

    if (!hasOnlyInitialColors) {
        console.error('Color initialization error: Unexpected colors present');
        // Force reset of colors
        colorManager.availableColors = new Set(expectedInitialColors);
        console.log('Forced color reset to:', Array.from(colorManager.availableColors));
    }

    // Start audio initialization in parallel
    initAudio();

    const canvas = renderer.domElement;
    if (!canvas) {
        console.error("initGame: Canvas not found!");
        return;
    }
    console.log("initGame: Canvas found.");

    // Renderer is assumed to be already created and configured.
    renderer.shadowMap.enabled = true;
    renderer.shadowMap.type = THREE.PCFSoftShadowMap;
    renderer.autoClear = false; 
    console.log("initGame: Renderer configured.");

    // Initialize scene manager
    const sceneManager = new SceneManager();
    const scene = sceneManager.getScene();
    console.log("initGame: SceneManager created.");

    // Set up camera
    const camera = new THREE.PerspectiveCamera(
        GameConfig.CAMERA.FOV,
        GameConfig.RENDERER.WIDTH / GameConfig.RENDERER.HEIGHT,
        GameConfig.CAMERA.NEAR,
        GameConfig.CAMERA.FAR
    );
    console.log("initGame: Camera created.");
    
    // Initialize controllers
    const cameraController = new CameraController(
        camera,
        GameConfig.CAMERA.INITIAL_DISTANCE,
        GameConfig.CAMERA.INITIAL_HEIGHT
    );
    const gameState = new GameState();
    console.log("initGame: Controllers created.");
    
    // Initial camera position
    cameraController.updatePosition();
    scene.add(camera);
    console.log("initGame: Camera positioned and added to scene.");

    // Initialize UI Scene (for score display, etc.)
    const uiScene = new UIScene(GameConfig.RENDERER.WIDTH, GameConfig.RENDERER.HEIGHT);
    console.log("initGame: UIScene created.");
    
    // Initialize Game Over UI using the new UI system
    const gameOverUI = new GameOverUI(GameConfig.RENDERER.WIDTH, GameConfig.RENDERER.HEIGHT);
    window.gameOverUI = gameOverUI;
    console.log("initGame: GameOverUI created.");

    // Initialize draggable manager with UI Scene
    const draggableManager = new DraggableManager(camera, gameState, uiScene);
    window.draggableManager = draggableManager;
    console.log("initGame: DraggableManager created.");

    // Create and add the game board
    const gameBoard = new GameBoard(GameConfig.BOARD.HEX_RADIUS, GameConfig.BOARD.GAP_FACTOR);
    const board = gameBoard.createBoard();
    scene.add(board);
    console.log("initGame: Game board created and added to scene.");

    // Raycaster for mouse interaction
    const raycaster = new THREE.Raycaster();
    console.log("initGame: Raycaster created.");

    // Initialize event handler
    const eventHandler = new EventHandler(
        canvas, 
        gameState, 
        cameraController, 
        gameBoard, 
        scene, 
        raycaster,
        uiScene,      // Pass UIScene instance
        gameOverUI    // Pass GameOverUI instance
    );
    console.log("initGame: EventHandler created.");

    // Function to restart the game
    function restartGame() {
        console.log('Restarting game...');
        
        // Cancel all animation frames
        if (window.animationFrameId) {
            cancelAnimationFrame(window.animationFrameId);
            window.animationFrameId = null;
        }

        // Clean up THREE.js resources
        if (scene) {
            // Dispose of all geometries and materials
            scene.traverse(function(object) {
                if (object.geometry) {
                    object.geometry.dispose();
                }
                if (object.material) {
                    if (Array.isArray(object.material)) {
                        object.material.forEach(material => material.dispose());
                    } else {
                        object.material.dispose();
                    }
                }
            });
            
            // Clear the scene
            while(scene.children.length > 0) { 
                scene.remove(scene.children[0]); 
            }
        }

        // Dispose of the renderer
        if (renderer) {
            renderer.dispose();
            renderer = null;
        }

        // Clean up global state
        window.gameOverUI = null;
        window.draggableManager = null;
        window.audioManager = null;

        // Explicitly reset ColorManager singleton
        if (window.ColorManager) {
            ColorManager.instance = null;
            console.log('ColorManager singleton reset');
        }

        // Remove the old canvas
        const container = document.querySelector('.container');
        const oldCanvas = document.getElementById('glcanvas');
        oldCanvas.remove();

        // Create fresh canvas
        const newCanvas = document.createElement('canvas');
        newCanvas.id = 'glcanvas';
        newCanvas.width = GameConfig.RENDERER.WIDTH;
        newCanvas.height = GameConfig.RENDERER.HEIGHT;
        newCanvas.style.width = GameConfig.RENDERER.WIDTH + 'px';
        newCanvas.style.height = GameConfig.RENDERER.HEIGHT + 'px';
        container.appendChild(newCanvas);

        // Start fresh with bootstrap to show start screen
        startBootstrap();
    }

    window.restartGame = restartGame;

    // Initialize a clock for time delta
    const clock = new THREE.Clock();
    console.log("initGame: Clock created.");

    function animate() {
        window.animationFrameId = requestAnimationFrame(animate);
        const delta = clock.getDelta();

        renderer.clear(); 
        cameraController.handleRotation();
        draggableManager.updateDraggablePosition();
        
        renderer.render(scene, camera);
        
        renderer.autoClear = false;
        renderer.clearDepth();
        renderer.render(uiScene.scene, uiScene.camera);
        
        if (gameOverUI.visible) {
            renderer.render(gameOverUI.scene, gameOverUI.camera);
        }
        
        renderer.autoClear = true;
    }
    console.log("initGame: Starting animation loop.");
    animate(); // Start the main game animation

    console.log("initGame finished successfully.");
}

// Replace the existing bootstrap function with this updated version
function bootstrap() {
    console.log('Bootstrap function called');
    
    // Get the canvas and force its dimensions to match GameConfig
    const canvas = document.getElementById('glcanvas');
    if (!canvas) {
        console.error('Canvas element #glcanvas not found!');
        return;
    }
    canvas.width = GameConfig.RENDERER.WIDTH;
    canvas.height = GameConfig.RENDERER.HEIGHT;
    canvas.style.width = GameConfig.RENDERER.WIDTH + 'px';
    canvas.style.height = GameConfig.RENDERER.HEIGHT + 'px';

    // Check if StartScreenUI is defined
    if (typeof StartScreenUI === 'undefined') {
        console.error('StartScreenUI is undefined! ui.js might not have loaded correctly.');
        // Fallback: Maybe draw a simple error message on the canvas
        const ctx = canvas.getContext('2d');
        if (ctx) {
            ctx.fillStyle = 'red';
            ctx.fillRect(0, 0, canvas.width, canvas.height);
            ctx.fillStyle = 'white';
            ctx.font = '24px Arial';
            ctx.textAlign = 'center';
            ctx.fillText('Error: UI failed to load', canvas.width / 2, canvas.height / 2);
        }
        return; // Stop further execution
    }

    // Start bootstrap immediately, don't wait for session info
    startBootstrap();
    
    // Initialize game session in parallel - don't wait for it before showing UI
    initGameSession().catch(error => {
        console.error("Error during session initialization:", error);
        // The game will continue anyway, session might come later
    });
}

function startBootstrap() {
    console.log('Starting bootstrap...');
    
    const canvas = document.getElementById('glcanvas');
    if (!canvas) {
        console.error('Canvas element #glcanvas not found during startBootstrap!');
        return;
    }
    console.log('Canvas element found.');

    // *** Refined WebGL availability check ***
    let webglAvailable = false;
    if (typeof WEBGL !== 'undefined' && typeof WEBGL.isWebGLAvailable === 'function') {
        try {
            webglAvailable = WEBGL.isWebGLAvailable();
            console.log(`WEBGL.isWebGLAvailable() returned: ${webglAvailable}`);
        } catch (e) {
            console.error("Error calling WEBGL.isWebGLAvailable():", e);
            webglAvailable = false; // Assume not available if check throws error
        }
    } else {
        console.error("WEBGL object or WEBGL.isWebGLAvailable function is not defined.");
    }

    if (!webglAvailable) {
        console.error("WebGL check failed or returned false.");
        // Display error messages (using WEBGL helper if available)
        if (typeof WEBGL !== 'undefined' && typeof WEBGL.getWebGLErrorMessage === 'function') {
             document.body.appendChild(WEBGL.getWebGLErrorMessage());
        } else {
             // Append generic message
             const errorDiv = document.createElement('div');
             errorDiv.id = 'webgl-error-message';
             errorDiv.style.cssText = 'font-family:monospace; font-size:13px; font-weight:normal; text-align:center; background:#fff; color:#000; padding:1.5em; width:400px; margin:5em auto 0;';
             errorDiv.innerHTML = 'Your browser does not seem to support <a href="http://khronos.org/webgl/wiki/Getting_a_WebGL_Implementation" style="color:#000">WebGL</a>.<br/>Find out how to get it <a href="http://get.webgl.org/" style="color:#000">here</a>.';
             document.body.appendChild(errorDiv);
        }
         // Draw on canvas
         const ctx = canvas.getContext('2d');
         if (ctx) {
             ctx.fillStyle = 'black'; ctx.fillRect(0, 0, canvas.width, canvas.height);
             ctx.fillStyle = 'white'; ctx.font = '18px Arial'; ctx.textAlign = 'center';
             ctx.fillText('WebGL is required but not available.', canvas.width / 2, canvas.height / 2);
             ctx.fillText('Please enable WebGL or use a different browser.', canvas.width / 2, canvas.height / 2 + 30);
         }
        return; // Stop execution
    }
    console.log("WebGL check passed.");

    // Create renderer ONLY if check passed
    let renderer;
    try {
        console.log("Attempting to create WebGLRenderer...");
        // Force WebGL 1 context for broader compatibility initially
        const contextAttributes = { alpha: true, antialias: true, powerPreference: 'high-performance' };
        const context = canvas.getContext( 'webgl', contextAttributes ) || canvas.getContext( 'experimental-webgl', contextAttributes );
        if (!context) {
            throw new Error("Failed to get WebGL or experimental-webgl context directly.");
        }
        renderer = new THREE.WebGLRenderer({ 
            canvas: canvas,
            context: context, // Pass the context explicitly
            antialias: true,  // Kept for clarity, context attributes are primary
            alpha: true,
            powerPreference: 'high-performance'
        });
        renderer.setPixelRatio(window.devicePixelRatio);
        renderer.setSize(GameConfig.RENDERER.WIDTH, GameConfig.RENDERER.HEIGHT);
        renderer.setClearColor(GameConfig.RENDERER.BACKGROUND_COLOR);
        console.log("Renderer created successfully.");
    } catch (error) {
        console.error("Error creating WebGLRenderer:", error);
        if (typeof WEBGL !== 'undefined' && WEBGL.getWebGLErrorMessage) {
             document.body.appendChild(WEBGL.getWebGLErrorMessage());
        }
        return; // Stop if renderer fails
    }

    // Style the canvas for absolute positioning and scaling
    renderer.domElement.style.position = 'absolute';
    renderer.domElement.style.left = '0px';
    renderer.domElement.style.top = '0px';
    renderer.domElement.style.transformOrigin = 'top left';
    
    // Ensure body allows absolute positioning
    document.body.style.position = 'relative'; 
    document.body.style.overflow = 'hidden'; // Prevent scrollbars from scaled element

    document.body.appendChild(renderer.domElement);

    // --- Scaling Logic ---
    const gameCanvas = renderer.domElement;
    const GAME_WIDTH = GameConfig.RENDERER.WIDTH;
    const GAME_HEIGHT = GameConfig.RENDERER.HEIGHT;

    function applyScaling() {
        const availableWidth = window.innerWidth;
        const availableHeight = window.innerHeight;
        const baseScale = Math.min(availableWidth / GAME_WIDTH, availableHeight / GAME_HEIGHT);
        const finalScale = baseScale * 0.5; // Apply 50% scaling
        
        gameCanvas.style.transform = `scale(${finalScale})`;
        
        // Center the scaled canvas
        const offsetX = (availableWidth - (GAME_WIDTH * finalScale)) / 2;
        const offsetY = (availableHeight - (GAME_HEIGHT * finalScale)) / 2;
        // We already set position: absolute and transform-origin: top left
        gameCanvas.style.left = `${offsetX}px`;
        gameCanvas.style.top = `${offsetY}px`;
    }

    applyScaling(); // Initial scale
    window.addEventListener('resize', applyScaling); // Rescale on resize
    // --- End Scaling Logic ---

    // Create the Start Screen UI 
    let startUI;
    try {
        console.log("Attempting to create StartScreenUI...");
        startUI = new StartScreenUI(GameConfig.RENDERER.WIDTH, GameConfig.RENDERER.HEIGHT);
        startUI.visible = true;
        startUI.updateUI();
        console.log('StartScreenUI created successfully.');
    } catch (e) {
        console.error('Error creating StartScreenUI:', e);
        return; // Cannot proceed without UI
    }

    // Store a reference to the renderer for later use in updateUIForSession
    window.gameRenderer = renderer;
    window.gameStartUI = startUI;
    
    // Define the updateUIForSession function for when session arrives later
    window.updateUIForSession = () => {
        console.log("Updating UI for newly received session");
        // If we're still on the start screen, we can refresh button states
        if (startUI && startUI.visible && startUI.updateButtonStates) {
            startUI.updateButtonStates();
        }
        // If game is already running, we might want to update UI there too
    };

    // Animation loop for start screen
    let startScreenAnimationId;
    function animateStart() {
        startScreenAnimationId = requestAnimationFrame(animateStart);
        if (startUI && startUI.visible) {
            renderer.clear();
            renderer.render(startUI.scene, startUI.camera);
        } else {
            if (startScreenAnimationId) {
                cancelAnimationFrame(startScreenAnimationId);
                startScreenAnimationId = null; // Clear the ID
            }
        }
    }
    animateStart();

    // Add a variable to prevent double event firing
    let touchEventFired = false;
    let touchEventTimer = null;
    
    // Click handler for start button
    function onClickStart(event) {
        if (!startUI || !startUI.visible) return;

        // Check if this is a click event that happened right after a touch event
        if (event.type === 'click' && touchEventFired) {
            console.log('Ignoring click event that followed touch event');
            return;
        }

        let clientX, clientY;
        
        if (event.type === 'touchend') {
            // For touchend, we need to use changedTouches
            if (event.changedTouches && event.changedTouches.length > 0) {
                clientX = event.changedTouches[0].clientX;
                clientY = event.changedTouches[0].clientY;
                
                // Set the touch event flag to prevent the click event from firing
                touchEventFired = true;
                // Clear any existing timer
                if (touchEventTimer) clearTimeout(touchEventTimer);
                // Reset the flag after a short delay
                touchEventTimer = setTimeout(() => {
                    touchEventFired = false;
                    touchEventTimer = null;
                }, 300);
            } else {
                console.log("Touch event with no coordinates, ignoring");
                return;
            }
        } else {
            // For mouse events
            clientX = event.clientX;
            clientY = event.clientY;
        }

        console.log('onClickStart:', { 
            eventType: event.type, 
            clientX, 
            clientY, 
            buttonBounds: startUI.buttonBounds 
        });

        if (startUI.checkButtonClick(clientX, clientY, canvas)) {
            console.log('Start button clicked, initializing game...');
            
            // If we don't have a game session yet, notify the parent we're starting
            // This helps when session might come in immediately after starting
            if (!window.gameSession) {
                console.log('Starting game without session, notifying parent.');
                // Send a game_started message to parent so it knows we're going ahead
                try {
                    window.parent.postMessage({
                        type: 'game_started',
                        timestamp: Date.now()
                    }, '*');
                } catch (e) {
                    console.warn('Failed to send game_started message:', e);
                }
            }

            // Proceed with game initialization
            canvas.removeEventListener('click', onClickStart);
            canvas.removeEventListener('touchend', onClickStart);
            startUI.hide();
            
            if (startScreenAnimationId) {
                cancelAnimationFrame(startScreenAnimationId);
                startScreenAnimationId = null;
            }

            // Clean up start screen resources
            if (startUI.scene) {
                startUI.scene.traverse(function(object) {
                    if (object.geometry) object.geometry.dispose();
                    if (object.material) object.material.dispose();
                });
            }

            // Start the main game with the same renderer
            console.log('Calling initGame...');
            initGame(renderer);
        }
    }
    // Ensure listener is added only once
    canvas.removeEventListener('click', onClickStart); // Remove previous if any
    canvas.removeEventListener('touchend', onClickStart); // Remove previous if any
    canvas.addEventListener('click', onClickStart);
    canvas.addEventListener('touchend', onClickStart);

    console.log("startBootstrap finished setting up listeners and animation.");
}

// Start the bootstrap process when the window loads
window.addEventListener('load', bootstrap); 