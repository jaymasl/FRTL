class EventHandler {
    constructor(canvas, gameState, cameraController, gameBoard, scene, raycaster, uiScene, gameOverUI) {
        this.canvas = canvas;
        this.gameState = gameState;
        this.cameraController = cameraController;
        this.gameBoard = gameBoard;
        this.scene = scene;
        this.raycaster = raycaster;
        this.uiScene = uiScene;
        this.gameOverUI = gameOverUI;
        this.mouse = new THREE.Vector2();
        this.sortingManager = new SortingManager(gameBoard);
        
        this.setupEventListeners();
    }

    setupEventListeners() {
        this.canvas.addEventListener('mousedown', this.handleMouseDown.bind(this));
        this.canvas.addEventListener('mousemove', this.handleMouseMove.bind(this));
        this.canvas.addEventListener('mouseup', this.handleMouseUp.bind(this));
        this.canvas.addEventListener('mouseleave', this.handleMouseLeave.bind(this));
        this.canvas.addEventListener('touchstart', this.handleTouchStart.bind(this), { passive: false });
        this.canvas.addEventListener('touchmove', this.handleTouchMove.bind(this), { passive: false });
        this.canvas.addEventListener('touchend', this.handleTouchEnd.bind(this));
    }

    getNDC(clientX, clientY) {
        const rect = this.canvas.getBoundingClientRect();
        const ndc = {
            x: ((clientX - rect.left) / rect.width) * 2 - 1,
            y: -((clientY - rect.top) / rect.height) * 2 + 1
        };
        return ndc;
    }

    getIntersects(ndcX, ndcY) {
        this.raycaster.setFromCamera({ x: ndcX, y: ndcY }, this.cameraController.camera);
        return this.raycaster.intersectObjects(this.scene.children, true);
    }

    screenToBoardSpace(ndcX, ndcY) {
        this.raycaster.setFromCamera({ x: ndcX, y: ndcY }, this.cameraController.camera);
        const boardPlane = new THREE.Plane(new THREE.Vector3(0, 1, 0), 0);
        const intersection = new THREE.Vector3();
        this.raycaster.ray.intersectPlane(boardPlane, intersection);
        return intersection;
    }

    async handleInteractionStart(clientX, clientY, isTouchEvent = false) {
        console.log(`handleInteractionStart called. isTouchEvent: ${isTouchEvent}`);

        const ndc = this.getNDC(clientX, clientY);
        console.log(`Calculated NDC: x=${ndc.x.toFixed(3)}, y=${ndc.y.toFixed(3)}`);
        
        // Process other interactions only if the game is not over
        if (this.gameState.isGameOver) return;

        const intersects = this.getIntersects(ndc.x, ndc.y);
        let clickedObject = (intersects.length > 0) ? intersects[0].object : null;
        let isStack = false;
        let dashboardOptionClicked = null;
        
        while (clickedObject && clickedObject !== this.scene) {
            if (clickedObject.userData && clickedObject.userData.isDashboardOption) {
                dashboardOptionClicked = clickedObject;
                break;
            }
            for (let i = 0; i < this.gameState.draggableHexagons.length; i++) {
                const draggable = this.gameState.draggableHexagons[i];
                if (draggable && clickedObject === draggable.getMesh()) {
                    this.gameState.selectedDraggableIndex = i;
                    isStack = true;
                    break;
                }
            }
            if (isStack) break;
            clickedObject = clickedObject.parent;
        }
        
        if (dashboardOptionClicked) {
            const optionIndex = dashboardOptionClicked.userData.optionIndex;
            const chosenStack = window.draggableManager.selectDashboardOption(optionIndex);
            const emptyIndex = this.gameState.draggableHexagons.findIndex(d => !d);
            if (emptyIndex !== -1) {
                this.gameState.setDraggableHexagon(chosenStack, emptyIndex);
                this.gameState.selectedDraggableIndex = emptyIndex;
            }
            this.gameState.isMovingPiece = true;
            this.scene.attach(chosenStack.getMesh());
            const boardPos = this.screenToBoardSpace(ndc.x, ndc.y);
            chosenStack.getMesh().position.copy(boardPos);
            chosenStack.getMesh().rotation.copy(this.gameBoard.boardGroup.rotation);
            chosenStack.getMesh().scale.set(1, 1, 1);
        } else if (isStack) {
            if (window.audioManager) {
                await window.audioManager.resumeAudio();
            }
            this.gameState.isMovingPiece = true;
            const selectedStack = this.gameState.draggableHexagons[this.gameState.selectedDraggableIndex];
            const selectedMesh = selectedStack.getMesh();
            this.scene.attach(selectedMesh);
            const boardPos = this.screenToBoardSpace(ndc.x, ndc.y);
            selectedMesh.position.copy(boardPos);
            selectedMesh.rotation.copy(this.gameBoard.boardGroup.rotation);
            selectedMesh.scale.set(1, 1, 1);
        } else if (intersects.length > 0) {
            this.gameState.isDragging = true;
        }

        this.gameState.previousMousePosition = { x: clientX, y: clientY };
    }

    handleInteractionMove(clientX, clientY, isTouchEvent = false) {
        if (this.gameState.isGameOver) return;

        const ndc = this.getNDC(clientX, clientY);

        if (this.gameState.isMovingPiece && this.gameState.selectedDraggableIndex !== -1) {
            const selectedStack = this.gameState.draggableHexagons[this.gameState.selectedDraggableIndex];
            const selectedMesh = selectedStack.getMesh();
            const boardPos = this.screenToBoardSpace(ndc.x, ndc.y);
            const nearestPos = this.gameBoard.findNearestGridPosition(boardPos.x, boardPos.z);
            const dx = boardPos.x - nearestPos.x;
            const dz = boardPos.z - nearestPos.y;
            const distance = Math.sqrt(dx * dx + dz * dz);
            
            if (distance <= GameConfig.BOARD.PLACEMENT_THRESHOLD) {
                const existingStack = this.gameBoard.getPiecesAtPosition(nearestPos.x, nearestPos.y);
                if (existingStack) {
                    selectedMesh.position.copy(boardPos);
                } else {
                    const yPosition = GameConfig.BOARD.TILE_HEIGHT;
                    selectedMesh.position.set(nearestPos.x, yPosition, nearestPos.y);
                }
            } else {
                selectedMesh.position.copy(boardPos);
            }
            selectedMesh.rotation.copy(this.gameBoard.boardGroup.rotation);
        } else if (this.gameState.isDragging) {
            const deltaMove = {
                x: clientX - this.gameState.previousMousePosition.x,
                y: clientY - this.gameState.previousMousePosition.y
            };
            this.cameraController.updateAngle(deltaMove.x * 0.5);
        }

        this.gameState.previousMousePosition = { x: clientX, y: clientY };
    }

    async handleInteractionEnd(clientX, clientY, isTouchEvent = false) {
        if (this.gameState.isGameOver) {
            this.gameState.resetState();
            return;
        }

        const ndc = this.getNDC(clientX, clientY);
        
        if (this.gameState.isMovingPiece && this.gameState.selectedDraggableIndex !== -1) {
            const selectedStack = this.gameState.draggableHexagons[this.gameState.selectedDraggableIndex];
            const selectedMesh = selectedStack.getMesh();
            const boardPos = this.screenToBoardSpace(ndc.x, ndc.y);
            const nearestPos = this.gameBoard.findNearestGridPosition(boardPos.x, boardPos.z);
            const dx = boardPos.x - nearestPos.x;
            const dz = boardPos.z - nearestPos.y;
            const distance = Math.sqrt(dx * dx + dz * dz);
            
            if (distance <= GameConfig.BOARD.PLACEMENT_THRESHOLD) {
                const existingStack = this.gameBoard.getPiecesAtPosition(nearestPos.x, nearestPos.y);
                if (!existingStack) {
                    if (this.sortingManager.isProcessing) {
                        this.returnPieceToDashboard(selectedStack, selectedMesh);
                    } else {
                        const yPosition = GameConfig.BOARD.TILE_HEIGHT;
                        selectedMesh.position.set(nearestPos.x, yPosition, nearestPos.y);
                        this.gameBoard.addPieceToStack(nearestPos.x, nearestPos.y, selectedStack);
                        if (window.audioManager && window.audioManager.isReady()) {
                            window.audioManager.playSound('tile-place');
                        }
                        
                        this.gameState.setDraggableHexagon(null, this.gameState.selectedDraggableIndex);
                        
                        this.sortingManager.sortAt(nearestPos).then(async (sortingComplete) => {
                             await new Promise(resolve => setTimeout(resolve, 750));
                             if (sortingComplete) {
                                 const finalScoreCheck = await this.sortingManager.checkAllStacksForScoring();
                                 if (finalScoreCheck) {
                                     await new Promise(resolve => setTimeout(resolve, 750));
                                 }
                                 if (this.gameBoard.isGameOver()) {
                                     if (this.gameOverUI) {
                                         this.gameOverUI.show(this.sortingManager.score);
                                     }
                                     this.gameState.isGameOver = true;
                                 } else {
                                     const newDraggable = window.draggableManager.createNewDraggable();
                                     const emptyIndex = this.gameState.draggableHexagons.findIndex(d => d === null);
                                     if (emptyIndex !== -1) {
                                        this.gameState.setDraggableHexagon(newDraggable, emptyIndex);
                                        window.draggableManager.updateDraggablePosition();
                                     }
                                 }
                             }
                        }).catch(console.error);
                        console.log(`Placed stack at ${nearestPos.x},${nearestPos.y} with top color ${selectedStack.getTopColor()}`);
                    }
                } else {
                    this.returnPieceToDashboard(selectedStack, selectedMesh);
                }
            } else {
                this.returnPieceToDashboard(selectedStack, selectedMesh);
            }
        } else if (this.gameState.isDragging) {
            this.cameraController.startRotation(this.cameraController.cameraAngle);
        }
        this.gameState.resetState();
    }

    handleMouseDown(event) {
        console.log("handleMouseDown detected");
        this.handleInteractionStart(event.clientX, event.clientY);
    }

    handleMouseMove(event) {
        this.handleInteractionMove(event.clientX, event.clientY);
    }

    async handleMouseUp(event) {
        console.log("handleMouseUp detected");
        await this.handleInteractionEnd(event.clientX, event.clientY);
    }

    handleMouseLeave() {
        console.log("handleMouseLeave detected");
        if (this.gameState.isMovingPiece || this.gameState.isDragging) {
            this.handleInteractionEnd(this.gameState.previousMousePosition.x, this.gameState.previousMousePosition.y);
        }
        this.gameState.resetState();
    }

    handleTouchStart(event) {
        console.log("handleTouchStart detected");
        event.preventDefault();
        if (event.touches.length > 0) {
            this.handleInteractionStart(event.touches[0].clientX, event.touches[0].clientY, true);
        }
    }

    handleTouchMove(event) {
        event.preventDefault();
        if (event.touches.length > 0) {
            this.handleInteractionMove(event.touches[0].clientX, event.touches[0].clientY, true);
        }
    }

    async handleTouchEnd(event) {
        console.log("handleTouchEnd detected");
        if (event.changedTouches.length > 0) {
            await this.handleInteractionEnd(event.changedTouches[0].clientX, event.changedTouches[0].clientY, true);
        }
    }

    returnPieceToDashboard(stack, mesh) {
        if (mesh.parent) {
            mesh.parent.remove(mesh);
        }
        this.cameraController.camera.add(mesh);
        
        const index = this.gameState.draggableHexagons.findIndex(draggable => draggable === stack);
        if (index !== -1) {
             const pos = window.draggableManager.draggablePositions[index];
             mesh.position.set(pos.x, pos.y, pos.z);
             mesh.rotation.set(
                 GameConfig.DRAGGABLE.ROTATION.x,
                 GameConfig.DRAGGABLE.ROTATION.y,
                 GameConfig.DRAGGABLE.ROTATION.z
             );
             mesh.scale.set(
                 GameConfig.DRAGGABLE.SCALE.MINIMIZED,
                 GameConfig.DRAGGABLE.SCALE.MINIMIZED,
                 GameConfig.DRAGGABLE.SCALE.MINIMIZED
             );
             mesh.renderOrder = 10;
             this.gameState.updateDraggableStartPosition(mesh.position, index);
        } else {
            mesh.visible = false;
        }
    }
}

window.EventHandler = EventHandler; 