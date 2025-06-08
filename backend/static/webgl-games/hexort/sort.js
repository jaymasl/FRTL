"use strict";

class SortingManager {
    constructor(gameBoard) {
        this.gameBoard = gameBoard;
        this.sortingQueue = [];
        this.isProcessing = false;
        this.score = 0;
        this.lastMove = { from: null, to: null };
        this.lastScoreUpdate = 0; // Track when we last sent a score update
        this.scoreUpdateInterval = 5000; // Update score every 5 seconds
        this.lastScoreSent = 0; // Track the last score we sent to avoid unnecessary updates

        // Set up periodic score update
        this.scoreUpdateTimer = setInterval(() => {
            this.sendScoreUpdate();
        }, this.scoreUpdateInterval);

        // Store reference to this manager globally so it can be accessed for cleanup
        window.sortingManager = this;
        
        console.log('Periodic score update system initialized');
    }

    // Main entry point for sorting logic
    async sortAt(position) {
        if (!position) return;
        
        const targetStack = this.gameBoard.getPiecesAtPosition(position.x, position.y);
        if (!(targetStack instanceof DraggableHexagonStack) || targetStack.hexagons.length === 0) return;

        // Queue initial matches
        this.findMatches(position);
        
        // Process the entire turn
        if (!this.isProcessing) {
            this.isProcessing = true;
            // Set dashboard to sorting state
            if (window.draggableManager) {
                window.draggableManager.setSortingState(true);
            }
            await this.processTurn();
            this.isProcessing = false;
            // Reset dashboard state
            if (window.draggableManager) {
                window.draggableManager.setSortingState(false);
            }
            return true; // Return true when all processing is complete
        }
        return false;
    }

    // Process an entire turn including all moves, matches, and scoring
    async processTurn() {
        while (true) {
            // Process all current moves in the queue
            while (this.sortingQueue.length > 0) {
                const move = this.sortingQueue.shift();
                await this._processSingleMove(move);
            }

            // After all moves are done, check for scoring
            const scoreFound = await this.checkAllStacksForScoring();
            if (scoreFound) {
                // After scoring, scan for new matches
                await this._scanForNewMatches();
                // If new matches were found, continue the turn
                if (this.sortingQueue.length > 0) {
                    continue;
                }
            }

            // If no scoring occurred, scan for new matches
            if (!scoreFound) {
                await this._scanForNewMatches();
                // If new matches were found, continue the turn
                if (this.sortingQueue.length > 0) {
                    continue;
                }

                // No new matches, do one final scoring check
                const finalScoreFound = await this.checkAllStacksForScoring();
                if (finalScoreFound) {
                    continue;
                }
            }

            // If we get here, the turn is complete
            break;
        }
    }

    // Helper function to scan the board for new matches
    async _scanForNewMatches() {
        const checkedPositions = new Set();
        
        for (const posKey in this.gameBoard.stacks) {
            if (this.gameBoard.stacks[posKey]) {
                const [x, y] = posKey.split(',').map(Number);
                if (!checkedPositions.has(posKey)) {
                    this.findMatches({x, y});
                    checkedPositions.add(posKey);
                }
            }
        }
    }

    // Process a single move including animation and updates
    async _processSingleMove(move) {
        const sourceStack = this.gameBoard.getPiecesAtPosition(move.from.x, move.from.y);
        const targetStack = this.gameBoard.getPiecesAtPosition(move.to.x, move.to.y);
        
        if (!sourceStack || !targetStack || sourceStack === targetStack) {
            return;
        }

        // Double check that colors still match at time of move using the top of the source stack
        const sourceColor = sourceStack.getTopColor();
        const targetColor = targetStack.getTopColor();

        if (sourceColor !== targetColor || sourceColor !== move.color) {
            console.log('Move no longer valid - colors don\'t match:', { sourceColor, targetColor, expectedColor: move.color });
            return;
        }

        // Only take the top piece from the source stack
        if (sourceStack.hexagons.length > 0) {
            const pieceData = sourceStack.hexagons.pop();
            const pieceMesh = pieceData.hexagon.getMesh();
            
            // Store the original world position before removing from source stack
            const sourceWorldPos = new THREE.Vector3();
            pieceMesh.getWorldPosition(sourceWorldPos);
            
            sourceStack.group.remove(pieceMesh);
            
            // Add to target stack but maintain original world position
            targetStack.group.add(pieceMesh);
            const targetWorldMatrix = targetStack.group.matrixWorld;
            const targetWorldMatrixInverse = new THREE.Matrix4().copy(targetWorldMatrix).invert();
            const localPos = sourceWorldPos.clone().applyMatrix4(targetWorldMatrixInverse);
            pieceMesh.position.copy(localPos);

            // Calculate end position
            const stackIndex = targetStack.hexagons.length;
            const endPos = new THREE.Vector3(
                0,
                0,
                -stackIndex * pieceData.hexagon.getTotalHeight()
            );

            // Calculate lift height based on stack heights
            const sourceHeight = -localPos.z;
            const targetHeight = -endPos.z;
            const liftHeight = targetHeight > sourceHeight ? (targetHeight - sourceHeight) + 0.5 : 0;

            // Create and wait for animation
            if (!this.animationManager) {
                this.animationManager = new AnimationManager();
            }
            
            await this.animationManager.createMoveAnimation(
                pieceMesh,
                localPos,
                endPos,
                125,
                liftHeight
            );

            // After animation completes, update the stacks
            targetStack.hexagons.push(pieceData);
            
            // Update timestamp to mark this stack as most recently modified
            targetStack.lastModified = Date.now();

            // Play match sound when moving to same color
            if (window.audioManager && targetStack.hexagons.length > 1 && 
                targetStack.hexagons[targetStack.hexagons.length - 2].color === pieceData.color) {
                window.audioManager.playSound('tile-match');
            }
            
            // Update heights
            sourceStack.stackHeight = sourceStack.hexagons.reduce((height, piece) => 
                height + piece.hexagon.getTotalHeight(), 0);
            targetStack.stackHeight = targetStack.hexagons.reduce((height, piece) => 
                height + piece.hexagon.getTotalHeight(), 0);

            // If source stack is now empty, remove it and reset its height
            if (sourceStack.hexagons.length === 0) {
                this.gameBoard.boardGroup.remove(sourceStack.group);
                this.gameBoard.stacks[`${move.from.x.toFixed(6)},${move.from.y.toFixed(6)}`] = null;
                this.gameBoard.stackHeights[`${move.from.x.toFixed(6)},${move.from.y.toFixed(6)}`] = 0;
            }

            // Additional check: if source stack still has a matching top, queue another move from source to target
            if (sourceStack.hexagons.length > 0 && sourceStack.getTopColor() === move.color) {
                // Only queue the move if the target stack was modified more recently than the source stack
                const sourceTimestamp = sourceStack.lastModified || 0;
                const targetTimestamp = targetStack.lastModified || 0;
                if (targetTimestamp > sourceTimestamp) {
                    this.sortingQueue.push({
                        from: move.from,
                        to: move.to,
                        color: move.color
                    });
                }
            }

            // Update the new stack's timestamps as current
            targetStack.timestamp = Date.now();
            targetStack.lastModified = Date.now();
            
            // Insert slight delay to ensure new piece state is fully updated
            await new Promise(resolve => setTimeout(resolve, 50));
            
            // Reposition all pieces in the target stack to remove gaps using cumulative height
            let cumulativeHeight = 0;
            for (let i = 0; i < targetStack.hexagons.length; i++) {
                const pieceMesh = targetStack.hexagons[i].hexagon.getMesh();
                pieceMesh.position.z = -cumulativeHeight;
                cumulativeHeight += targetStack.hexagons[i].hexagon.getTotalHeight();
            }
            
            // Queue new matches from the target position only
            this.findMatches(move.to);
        }
    }

    // Find all matches for a position
    findMatches(position) {
        const targetStack = this.gameBoard.getPiecesAtPosition(position.x, position.y);
        
        if (!(targetStack instanceof DraggableHexagonStack) || targetStack.hexagons.length === 0) {
            return;
        }

        const targetColor = targetStack.getTopColor();
        
        // Get all directly adjacent neighbors
        const neighbors = this.gameBoard.findNeighborPositions(position.x, position.y);
        const matchingStacks = [];
        
        // Check each neighbor for a matching color
        for (const neighbor of neighbors) {
            const neighborStack = this.gameBoard.getPiecesAtPosition(neighbor.x, neighbor.y);
            if (neighborStack instanceof DraggableHexagonStack && 
                neighborStack.hexagons.length > 0 && 
                neighborStack.getTopColor() === targetColor &&
                neighborStack !== targetStack) {
                
                // Only move pieces TO the more recently modified stack
                if (!targetStack.lastModified || 
                    !neighborStack.lastModified || 
                    targetStack.lastModified > neighborStack.lastModified) {
                    matchingStacks.push({
                        stack: neighborStack,
                        position: neighbor
                    });
                } else {
                continue;
            }
            }
        }

        // Update the target stack's last match time
        targetStack.lastMatchTime = Date.now();

        // Process all matching stacks
        for (const match of matchingStacks) {
            this.sortingQueue.push({
                from: match.position,
                to: position,
                color: targetColor
            });
        }
    }

    // Update score
    updateScore(points) {
        const previousScore = this.score;
        console.log('updateScore: previousScore =', previousScore, 'points =', points);
        this.score += points;
        console.log('updateScore: newScore =', this.score);
        
        // Update WebGL score display
        if (window.draggableManager) {
            window.draggableManager.updateScoreDisplay(this.score);
        }

        // Send a score update immediately after a significant score change (5+ points)
        if (points >= 5) {
            this.sendScoreUpdate();
        }
        
        // Check if we've crossed any score thresholds
        const thresholds = Object.keys(SCORE_THRESHOLDS).map(Number);
        const previousThreshold = Math.max(...thresholds.filter(t => t <= previousScore));
        const currentThreshold = Math.max(...thresholds.filter(t => t <= this.score));

        // If we've crossed a threshold, update available colors
        if (currentThreshold > previousThreshold) {
            const colorManager = new ColorManager();
            colorManager.updateAvailableColors(this.score);
            console.log(`Crossed score threshold! New colors unlocked at score ${this.score}`);
        }
    }

    // Check for scoring opportunities in a stack
    async checkStackForScoring(stack, position, shouldPlaySound = true) {
        if (!stack || stack.hexagons.length < 5) return false;

        let currentConsecutive = 1;
        let currentColor = stack.hexagons[0].color;
        let matchingSequences = [];
        let currentSequence = {
            color: currentColor,
            startIndex: 0,
            length: 1
        };

        // Find all sequences of consecutive same-colored hexagons
        for (let i = 1; i < stack.hexagons.length; i++) {
            if (stack.hexagons[i].color === currentColor) {
                currentConsecutive++;
                currentSequence.length = currentConsecutive;
            } else {
                if (currentConsecutive >= 5) {
                    matchingSequences.push({...currentSequence});
                }
                currentConsecutive = 1;
                currentColor = stack.hexagons[i].color;
                currentSequence = {
                    color: currentColor,
                    startIndex: i,
                    length: 1
                };
            }
        }

        // Check the last sequence
        if (currentConsecutive >= 5) {
            matchingSequences.push({...currentSequence});
        }

        if (matchingSequences.length === 0) return false;

        // Process all matching sequences from bottom to top
        matchingSequences.sort((a, b) => b.startIndex - a.startIndex);
        
        let totalPoints = 0;
        for (const sequence of matchingSequences) {
            const startIndex = sequence.startIndex;
            const length = sequence.length;
            const color = sequence.color;
            
            // Remove the hexagons
            const piecesToRemove = stack.hexagons.splice(startIndex, length);
            
            // Calculate points based on color
            let pointsPerPiece;
            switch(color) {
                case HEXAGON_COLORS.RED:
                case HEXAGON_COLORS.GREEN:
                case HEXAGON_COLORS.BLUE:
                    pointsPerPiece = 1;
                    break;
                case HEXAGON_COLORS.YELLOW:
                    pointsPerPiece = 2;
                    break;
                case HEXAGON_COLORS.PURPLE:
                    pointsPerPiece = 3;
                    break;
                case HEXAGON_COLORS.CYAN:
                    pointsPerPiece = 4;
                    break;
                case HEXAGON_COLORS.PINK:
                    pointsPerPiece = 5;
                    break;
                default:
                    pointsPerPiece = 1;
            }
            
            totalPoints += length * pointsPerPiece;
            
            // Play score sound immediately if this is the first sequence
            if (shouldPlaySound && window.audioManager && window.audioManager.isReady()) {
                window.audioManager.playSound('score', 0.7);
                shouldPlaySound = false; // Only play once per scoring check
            }

            // Add glow effect to each piece before removing
            const glowPromises = piecesToRemove.map(piece => {
                return new Promise(resolve => {
                    const glowEffect = new GlowEffect(piece.hexagon.getMesh(), color, 1000);
                    glowEffect.start();
                    
                    // Create animation loop for this specific glow effect
                    function updateGlow() {
                        if (glowEffect.update()) {
                            requestAnimationFrame(updateGlow);
                        } else {
                            resolve();
                        }
                    }
                    updateGlow();
                });
            });

            // Start all glow effects simultaneously and wait for them to complete
            await Promise.all(glowPromises);
            
            // Remove the meshes after glow effect
            for (const piece of piecesToRemove) {
                const pieceMesh = piece.hexagon.getMesh();
                stack.group.remove(pieceMesh);
                // Properly dispose of materials and geometries
                if (pieceMesh.material) {
                    if (Array.isArray(pieceMesh.material)) {
                        pieceMesh.material.forEach(m => m.dispose());
                    } else {
                        pieceMesh.material.dispose();
                    }
                }
                if (pieceMesh.geometry) {
                    pieceMesh.geometry.dispose();
                }
            }
        }

        // Reposition remaining hexagons
        for (let i = 0; i < stack.hexagons.length; i++) {
            const pieceMesh = stack.hexagons[i].hexagon.getMesh();
            pieceMesh.position.z = -i * stack.hexagons[i].hexagon.getTotalHeight();
        }

        // Update stack height
        stack.stackHeight = stack.hexagons.reduce((height, piece) => 
            height + piece.hexagon.getTotalHeight(), 0);
        
        // Update score with the new point total
        this.updateScore(totalPoints);
        console.log(`Score increased by ${totalPoints}! Total score: ${this.score}`);

        // If stack is empty after removal, clean it up
        if (stack.hexagons.length === 0) {
            this.gameBoard.boardGroup.remove(stack.group);
            this.gameBoard.stacks[`${position.x.toFixed(6)},${position.y.toFixed(6)}`] = null;
            this.gameBoard.stackHeights[`${position.x.toFixed(6)},${position.y.toFixed(6)}`] = 0;
        } else {
            // Reset the lastMatchTime to ensure this stack is treated as newly active
            stack.lastMatchTime = Date.now() + 1;  // Add 1ms to ensure it's the most recent

            // Find matches for this stack and all its neighbors
            this.findMatches(position);
            
            // Also check neighbors for potential matches with the new top color
            const neighbors = this.gameBoard.findNeighborPositions(position.x, position.y);
            for (const neighbor of neighbors) {
                const neighborStack = this.gameBoard.getPiecesAtPosition(neighbor.x, neighbor.y);
                if (neighborStack) {
                    this.findMatches({x: neighbor.x, y: neighbor.y});
                }
            }
        }

        return true;
    }

    // Check all stacks for scoring after sorting is complete
    async checkAllStacksForScoring() {
        let scoreFound = false;
        let isFirstScore = true;

        for (const posKey in this.gameBoard.stacks) {
            const stack = this.gameBoard.stacks[posKey];
            if (stack) {
                const [x, y] = posKey.split(',').map(Number);
                // Only play sound for the first scoring stack
                if (await this.checkStackForScoring(stack, {x, y}, isFirstScore)) {
                    scoreFound = true;
                    isFirstScore = false;
                }
            }
        }
        return scoreFound;
    }

    /**
     * Send a score update to the parent window for leaderboard updates
     * Only sends if the score has changed since last update
     */
    sendScoreUpdate() {
        // Only send if score has changed since last update
        if (this.score !== this.lastScoreSent) {
            try {
                window.parent.postMessage({
                    type: 'GAME_SCORE_UPDATE',
                    score: this.score,
                    timestamp: Math.floor(Date.now() / 1000),
                    is_final: false, // Flag to indicate this is not the final score
                    session_id: window.gameSession || 'unknown' // Include the session token
                }, '*');
                console.log('Score update sent to parent for leaderboard:', this.score);
                this.lastScoreSent = this.score;
            } catch (e) {
                console.error('Error sending periodic score update:', e);
            }
        }
    }
}

// Export the SortingManager
window.SortingManager = SortingManager; 