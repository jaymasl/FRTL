"use strict";

// Game configuration and constants
const HEXAGON_COLORS = {
    RED: 0xff0000,
    GREEN: 0x008000,
    BLUE: 0x0000ff,
    YELLOW: 0xffff00,
    PURPLE: 0x800080,
    CYAN: 0x00ffff,
    PINK: 0xff69b4
};

const SCORE_THRESHOLDS = {
    0: ['RED', 'GREEN', 'BLUE'],
    20: ['YELLOW'],
    60: ['PURPLE'],
    180: ['CYAN'],
    420: ['PINK']
};

const INTERACTION_TYPES = {
    MERGE: 'merge',
    CHAIN: 'chain',
    EXPLODE: 'explode'
};

// Effect system
class HexagonEffect {
    constructor(type, params = {}) {
        this.type = type;
        this.params = params;
        this.isActive = false;
        this.progress = 0;
    }

    apply(source, target) {
        this.isActive = true;
        this.progress = 0;
    }

    update(deltaTime) {
        if (!this.isActive) return false;
        this.progress += deltaTime;
        return this.progress < 1;
    }

    finish() {
        this.isActive = false;
        this.progress = 0;
    }
}

// Color management
class ColorManager {
    static instance = null;

    constructor() {
        if (ColorManager.instance) {
            return ColorManager.instance;
        }
        ColorManager.instance = this;
        
        this.colors = HEXAGON_COLORS;
        this.colorRules = new Map();
        this.resetToInitialState();
        this.setupDefaultRules();
        
        console.log('New ColorManager created with colors:', Array.from(this.availableColors));
    }

    resetToInitialState() {
        // Always start with only the initial colors (RED, GREEN, BLUE)
        this.availableColors = new Set(['RED', 'GREEN', 'BLUE']);
        console.log('ColorManager reset to initial state:', Array.from(this.availableColors));
    }

    setupDefaultRules() {
        // Set up merge rules for all possible color combinations
        Object.keys(HEXAGON_COLORS).forEach(color1 => {
            Object.keys(HEXAGON_COLORS).forEach(color2 => {
                if (color1 === color2) {
                    this.addColorRule(
                        HEXAGON_COLORS[color1],
                        HEXAGON_COLORS[color2],
                        INTERACTION_TYPES.MERGE
                    );
                }
            });
        });
    }

    updateAvailableColors(score) {
        // First reset to initial state
        this.resetToInitialState();
        
        // Then add colors based on score thresholds
        Object.entries(SCORE_THRESHOLDS)
            .sort(([a], [b]) => Number(a) - Number(b))
            .forEach(([threshold, colors]) => {
                if (score >= Number(threshold)) {
                    colors.forEach(color => this.availableColors.add(color));
                }
            });
        
        console.log('Available colors updated for score', score, ':', Array.from(this.availableColors));
    }

    getRandomAvailableColor() {
        const availableColorsList = Array.from(this.availableColors);
        const randomIndex = Math.floor(Math.random() * availableColorsList.length);
        const colorName = availableColorsList[randomIndex];
        return HEXAGON_COLORS[colorName];
    }

    addColorRule(color1, color2, interactionType) {
        const num1 = parseInt(color1, 10);
        const num2 = parseInt(color2, 10);
        const key = `${Math.min(num1, num2)},${Math.max(num1, num2)}`;
        this.colorRules.set(key, interactionType);
    }

    getInteractionType(color1, color2) {
        const num1 = parseInt(color1, 10);
        const num2 = parseInt(color2, 10);
        const key = `${Math.min(num1, num2)},${Math.max(num1, num2)}`;
        return this.colorRules.get(key);
    }
}

// Base hexagon class
class Hexagon {
    constructor(radius = 0.5, color = HEXAGON_COLORS.GREEN, depth = 0.05) {
        this.createGeometry(radius, depth);
        this.createMaterial(color);
        this.createMesh();
        this.setupInteractionState();
    }

    createGeometry(radius, depth) {
        const shape = new THREE.Shape();
        const angleOffset = Math.PI / 2;
        
        for (let i = 0; i < 6; i++) {
            const angle = angleOffset + i * (Math.PI / 3);
            const x = radius * Math.cos(angle);
            const y = radius * Math.sin(angle);
            i === 0 ? shape.moveTo(x, y) : shape.lineTo(x, y);
        }
        shape.closePath();

        const extrudeSettings = {
            depth: depth,
            bevelEnabled: true,
            bevelThickness: depth / 2,
            bevelSize: depth * 0.4,
            bevelSegments: 6,
            curveSegments: 6
        };

        this.geometry = new THREE.ExtrudeGeometry(shape, extrudeSettings);
        this.geometry.computeVertexNormals();
        
        this.depth = depth;
        this.totalHeight = depth + (depth / 2) * 2;
    }

    createMaterial(color) {
        this.material = new THREE.MeshPhongMaterial({
            color: color,
            shininess: 40,
            specular: 0x333333,
            flatShading: false,
            side: THREE.DoubleSide
        });
    }

    createMesh() {
        this.mesh = new THREE.Mesh(this.geometry, this.material);
        this.group = new THREE.Group();
        this.group.add(this.mesh);
    }

    setupInteractionState() {
        this.isDraggable = false;
        this.interactionState = {
            isActive: false,
            interactingWith: null,
            effect: null
        };
    }

    setPosition(x, y, z = 0) {
        this.group.position.set(x, y, z);
        return this;
    }

    setRotation(x, y, z) {
        this.group.rotation.set(x, y, z);
        return this;
    }

    getMesh() {
        return this.group;
    }

    setColor(color) {
        this.material.color.setHex(color);
        return this;
    }

    getColor() {
        return this.material.color.getHex();
    }

    setDraggable(draggable) {
        this.isDraggable = draggable;
        return this;
    }

    getTotalHeight() {
        return this.totalHeight;
    }

    startInteraction(otherHexagon, effect) {
        this.interactionState.isActive = true;
        this.interactionState.interactingWith = otherHexagon;
        this.interactionState.effect = effect;
        return this;
    }

    stopInteraction() {
        this.interactionState.isActive = false;
        this.interactionState.interactingWith = null;
        this.interactionState.effect = null;
        return this;
    }
}

// Board tile representation
class HexagonBoardTile {
    constructor(x, y, z = 0, hexRadius = 0.4, color = 0x333333) {
        this.hexagon = new Hexagon(hexRadius, color, 0.1);
        this.hexagon.setPosition(x, y, z);
    }

    getMesh() {
        return this.hexagon.getMesh();
    }

    getTotalHeight() {
        return this.hexagon.getTotalHeight();
    }
}

// Stack of draggable hexagons
class DraggableHexagonStack {
    constructor(hexRadius = 0.4, count = null) {
        this.group = new THREE.Group();
        this.hexagons = [];
        this.colorManager = new ColorManager();
        // If count is not provided or exceeds max, use a random number between 1 and 4
        const maxStackSize = 4;
        const stackSize = count ? Math.min(count, maxStackSize) : Math.floor(Math.random() * maxStackSize) + 1;
        this.createStack(hexRadius, stackSize);
    }

    createStack(hexRadius, count) {
        const pieceDepth = 0.05;
        const hex = new Hexagon(hexRadius, HEXAGON_COLORS.RED, pieceDepth);
        const pieceHeight = hex.getTotalHeight();
        this.stackHeight = count * pieceHeight;

        for (let i = 0; i < count; i++) {
            const randomColor = this.colorManager.getRandomAvailableColor();
            const hex = new Hexagon(hexRadius, randomColor, pieceDepth);
            this.hexagons.push({
                hexagon: hex,
                color: randomColor
            });
            hex.setDraggable(true);
            hex.getMesh().position.z = -i * pieceHeight;
            this.group.add(hex.getMesh());
        }
    }

    getMesh() {
        return this.group;
    }

    getColors() {
        return this.hexagons.map(h => h.color);
    }

    getTopColor() {
        return this.hexagons[this.hexagons.length - 1]?.color || null;
    }

    getTopHexagon() {
        return this.hexagons[this.hexagons.length - 1]?.hexagon || null;
    }

    getColorCount(color) {
        return this.hexagons.filter(h => h.color === color).length;
    }

    setDraggable(draggable) {
        this.hexagons.forEach(h => h.hexagon.setDraggable(draggable));
    }

    getPieceHeight() {
        return this.hexagons[0]?.hexagon.getTotalHeight() || 0;
    }

    canInteractWith(otherStack) {
        if (!otherStack || !this.hexagons.length || !otherStack.hexagons.length) return false;
        const interactionType = this.colorManager.getInteractionType(this.getTopColor(), otherStack.getTopColor());
        return interactionType !== undefined;
    }

    startInteractionWith(otherStack) {
        if (!this.canInteractWith(otherStack)) return null;
        
        const interactionType = this.colorManager.getInteractionType(this.getTopColor(), otherStack.getTopColor());
        const effect = new HexagonEffect(interactionType);
        
        this.getTopHexagon().startInteraction(otherStack.getTopHexagon(), effect);
        return effect;
    }

    toString() {
        return `Stack at ${this.group.position.x.toFixed(3)},${this.group.position.z.toFixed(3)} 
                with ${this.hexagons.length} pieces (top: ${this.getTopColor()})`;
    }
}

// Game board management
class GameBoard {
    constructor(hexRadius = 0.4, gapFactor = 1.1) {
        this.hexRadius = hexRadius;
        this.gapFactor = gapFactor;
        this.horizontalSpacing = hexRadius * Math.sqrt(3) * gapFactor;
        this.setupBoard();
    }

    setupBoard() {
        this.boardGroup = new THREE.Group();
        this.boardGroup.rotation.x = Math.PI / 2;
        
        this.gridPositions = [];
        this.stackHeights = {};
        this.stacks = {};
        this.neighbors = {};
        
        this.boardTileHeight = 0.2;
        this.tileInstance = new HexagonBoardTile(0, 0, 0, this.hexRadius);
        this.actualBoardHeight = this.tileInstance.getTotalHeight();
    }

    _getPositionKey(x, y) {
        return `${x.toFixed(6)},${y.toFixed(6)}`;
    }

    addTile(x, y, z = 0, color) {
        const roundedX = parseFloat(x.toFixed(6));
        const roundedY = parseFloat(y.toFixed(6));
        const tile = new HexagonBoardTile(roundedX, roundedY, z, this.hexRadius, color);
        this.boardGroup.add(tile.getMesh());
        const posKey = this._getPositionKey(roundedX, roundedY);
        this.gridPositions.push({ x: roundedX, y: roundedY, z });
        this.stacks[posKey] = null;
        this.stackHeights[posKey] = 0;
    }

    createBoard() {
        // Center tile
        this.addTile(0, 0);
        
        // First ring (6 tiles)
        for (let i = 0; i < 6; i++) {
            const angle = i * (Math.PI / 3);
            const x = parseFloat((this.horizontalSpacing * Math.cos(angle)).toFixed(6));
            const y = parseFloat((this.horizontalSpacing * Math.sin(angle)).toFixed(6));
            this.addTile(x, y);
        }
        
        // Second ring (12 tiles)
        for (let i = 0; i < 12; i++) {
            const angle = i * (Math.PI / 6);
            const radius = (i % 2 === 0) ? this.horizontalSpacing * 2 : this.horizontalSpacing * Math.sqrt(3);
            const x = parseFloat((radius * Math.cos(angle)).toFixed(6));
            const y = parseFloat((radius * Math.sin(angle)).toFixed(6));
            this.addTile(x, y);
        }

        this.calculateNeighbors();
        return this.boardGroup;
    }

    addPieceToStack(x, y, stack) {
        const posKey = this._getPositionKey(x, y);

        // Update the new stack's timestamp to ensure it's considered the newest
        stack.timestamp = Date.now();
        stack.lastModified = Date.now();

        // Ensure that if an existing value is present but not a valid stack, reset it
        if (this.stacks[posKey] && !this.stacks[posKey].hexagons) {
            this.stacks[posKey] = null;
        }

        if (!this.stacks[posKey] || !this.stacks[posKey].hexagons) {
            // No existing stack, so add the new stack normally
            this.stacks[posKey] = stack;
            // Position the stack above the board tile with adjusted height
            stack.group.position.set(x, y, -this.actualBoardHeight + 0.1);
            stack.group.rotation.set(0, 0, 0);
            this.boardGroup.add(stack.group);
            this.stackHeights[posKey] = stack.stackHeight;
        } else {
            // There is an existing stack
            const existingStack = this.stacks[posKey];
            // Compare timestamps: if the new stack is more recent, merge the older existing stack into the new stack
            if (stack.timestamp > existingStack.timestamp) {
                // Merge existingStack into the new stack
                while (existingStack.hexagons.length > 0) {
                    const pieceData = existingStack.hexagons.shift();
                    existingStack.group.remove(pieceData.hexagon.getMesh());
                    stack.hexagons.push(pieceData);
                    const stackIndex = stack.hexagons.length - 1;
                    pieceData.hexagon.getMesh().position.z = -stackIndex * pieceData.hexagon.getTotalHeight();
                    stack.group.add(pieceData.hexagon.getMesh());
                }
                stack.stackHeight = stack.hexagons.reduce((height, piece) => 
                    height + piece.hexagon.getTotalHeight(), 0);
                this.boardGroup.remove(existingStack.group);
                this.stacks[posKey] = stack;
                this.stackHeights[posKey] = stack.stackHeight;
                // Update the new stack's timestamps as current
                stack.timestamp = Date.now();
                stack.lastModified = Date.now();
            } else {
                // Otherwise, merge the new stack into the existing stack
                existingStack.timestamp = Date.now();
                existingStack.lastModified = Date.now();
                while (stack.hexagons && stack.hexagons.length > 0) {
                    const pieceData = stack.hexagons.pop();
                    const pieceMesh = pieceData.hexagon.getMesh();
                    stack.group.remove(pieceMesh);
                    existingStack.hexagons.push(pieceData);
                    const stackIndex = existingStack.hexagons.length - 1;
                    pieceMesh.position.z = -stackIndex * pieceData.hexagon.getTotalHeight();
                    existingStack.group.add(pieceMesh);
                }
                existingStack.stackHeight = existingStack.hexagons.reduce((height, piece) => 
                    height + piece.hexagon.getTotalHeight(), 0);
                this.stackHeights[posKey] = existingStack.stackHeight;
            }
        }
    }

    getPiecesAtPosition(x, y) {
        const posKey = this._getPositionKey(x, y);
        return this.stacks[posKey] || null;
    }

    getStackHeight(x, y) {
        const key = this._getPositionKey(x, y);
        return this.stackHeights[key] || 0;
    }

    addStackHeight(x, y, height) {
        const key = this._getPositionKey(x, y);
        this.stackHeights[key] = (this.stackHeights[key] || 0) + height;
    }

    findNeighborPositions(x, y) {
        const neighbors = [];
        const baseAngles = [0, Math.PI/3, 2*Math.PI/3, Math.PI, 4*Math.PI/3, 5*Math.PI/3];
        
        // Check both clockwise and counter-clockwise positions
        for (const angle of baseAngles) {
            const nx = x + this.horizontalSpacing * Math.cos(angle);
            const ny = y + this.horizontalSpacing * Math.sin(angle);
            const nearestPos = this.findNearestGridPosition(nx, ny);
            if (nearestPos && !neighbors.some(p => 
                p.x.toFixed(6) === nearestPos.x.toFixed(6) && 
                p.y.toFixed(6) === nearestPos.y.toFixed(6))
            ) {
                neighbors.push(nearestPos);
            }
        }
        return neighbors;
    }

    calculateNeighbors() {
        for (const pos of this.gridPositions) {
            const posKey = this._getPositionKey(pos.x, pos.y);
            this.neighbors[posKey] = this.findNeighborPositions(pos.x, pos.y);
        }
    }

    findNearestGridPosition(worldX, worldY) {
        const roundedX = parseFloat(worldX.toFixed(6));
        const roundedY = parseFloat(worldY.toFixed(6));
        
        let nearest = null;
        let minDistance = Infinity;
        
        for (const pos of this.gridPositions) {
            const dx = roundedX - pos.x;
            const dy = roundedY - pos.y;
            const distance = Math.sqrt(dx * dx + dy * dy);
            
            if (distance < minDistance) {
                minDistance = distance;
                nearest = pos;
            }
        }
        
        return nearest;
    }

    checkForInteractions(position) {
        const neighbors = this.findNeighborPositions(position.x, position.y);
        const currentStack = this.getPiecesAtPosition(position.x, position.y);
        
        if (!(currentStack instanceof DraggableHexagonStack)) return [];

        const interactions = [];
        for (const neighbor of neighbors) {
            const neighborStack = this.getPiecesAtPosition(neighbor.x, neighbor.y);
            if (neighborStack instanceof DraggableHexagonStack) {
                if (currentStack.canInteractWith(neighborStack)) {
                    interactions.push({
                        position: neighbor,
                        stack: neighborStack
                    });
                }
            }
        }
        
        return interactions;
    }

    getBoardHeight() {
        return this.actualBoardHeight;
    }

    isGameOver() {
        // If board is completely empty, don't trigger game over
        const isEmpty = this.gridPositions.every(pos => {
            const posKey = this._getPositionKey(pos.x, pos.y);
            return this.stacks[posKey] === null;
        });
        if (isEmpty) return false;
        
        return this.areAllSpotsFilled() || !this.hasValidMoves();
    }

    areAllSpotsFilled() {
        // Check each grid position to see if it has a stack
        return this.gridPositions.every(pos => {
            const posKey = this._getPositionKey(pos.x, pos.y);
            return this.stacks[posKey] !== null;
        });
    }

    hasValidMoves() {
        const colorManager = new ColorManager();
        const availableColorNames = Array.from(colorManager.availableColors);
        const availableColorsHex = availableColorNames.map(name => colorManager.colors[name]);
        const emptyPositions = this.gridPositions.filter(pos => this.getPiecesAtPosition(pos.x, pos.y) === null);
        for (const emptyPos of emptyPositions) {
            const neighbors = this.findNeighborPositions(emptyPos.x, emptyPos.y);
            const neighborColors = neighbors.map(nPos => {
                const stack = this.getPiecesAtPosition(nPos.x, nPos.y);
                return stack ? stack.getTopColor() : null;
            }).filter(color => color !== null);
            for (const color of availableColorsHex) {
                if (neighborColors.includes(color)) {
                    return true;
                }
            }
        }
        return false;
    }

    static createGameBoard(hexRadius = 0.4, gapFactor = 1.1) {
        const gameBoard = new GameBoard(hexRadius, gapFactor);
        return gameBoard.createBoard();
    }
}

// Export classes
window.Hexagon = Hexagon;
window.HexagonBoardTile = HexagonBoardTile;
window.GameBoard = GameBoard;
window.DraggableHexagonStack = DraggableHexagonStack; 