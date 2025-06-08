/*
 * ui.js
 * This file contains UI classes for the game, including the start screen UI and the game over UI.
 */

if (typeof window.StartScreenUI === 'undefined') {
// StartScreenUI displays an initial start screen overlay with a "Start Playing" button
class StartScreenUI {
    constructor(width, height) {
        this.width = width;
        this.height = height;
        this.scene = new THREE.Scene();
        this.camera = new THREE.OrthographicCamera(width / -2, width / 2, height / 2, height / -2, 1, 10);
        this.camera.position.z = 5;

        this.canvas = document.createElement('canvas');
        this.canvas.width = width;
        this.canvas.height = height;
        this.context = this.canvas.getContext('2d');

        this.texture = new THREE.CanvasTexture(this.canvas);
        this.texture.minFilter = THREE.LinearFilter;

        const geometry = new THREE.PlaneGeometry(width, height);
        const material = new THREE.MeshBasicMaterial({
            map: this.texture,
            transparent: true,
            depthTest: false
        });
        this.plane = new THREE.Mesh(geometry, material);
        this.plane.renderOrder = 10000;
        this.scene.add(this.plane);

        this.visible = true;
        this.buttonBounds = {
            x: (width - 240) / 2,
            y: (height / 2) + 20,
            width: 240,
            height: 80
        };

        // Load background image
        this.backgroundImage = new Image();
        this.backgroundImage.onload = () => {
            this.updateUI();
        };
        this.backgroundImage.src = 'background1.jpg';
        
        // Initial draw
        this.updateUI();
    }

    updateUI() {
        // Clear the canvas
        this.context.clearRect(0, 0, this.width, this.height);
        
        // Draw background image if loaded
        if (this.backgroundImage.complete) {
            // Calculate scaling to cover the entire canvas while maintaining aspect ratio
            const scale = Math.max(
                this.width / this.backgroundImage.width,
                this.height / this.backgroundImage.height
            );
            
            const scaledWidth = this.backgroundImage.width * scale;
            const scaledHeight = this.backgroundImage.height * scale;
            
            // Center the image
            const x = (this.width - scaledWidth) / 2;
            const y = (this.height - scaledHeight) / 2;
            
            this.context.drawImage(
                this.backgroundImage,
                x, y,
                scaledWidth,
                scaledHeight
            );
        } else {
            // Fallback to black background if image hasn't loaded
            this.context.fillStyle = '#000';
            this.context.fillRect(0, 0, this.width, this.height);
        }
        
        // Draw dark overlay to ensure text visibility
        this.context.fillStyle = 'rgba(0, 0, 0, 0.4)';
        this.context.fillRect(0, 0, this.width, this.height);
        
        // Draw title text with black outline
        this.context.font = 'bold 48px Arial';
        this.context.textAlign = 'center';
        this.context.textBaseline = 'middle';
        this.context.lineWidth = 4;
        this.context.strokeStyle = 'black';
        this.context.strokeText('Hexagon Sorting Game', this.width / 2, this.height / 2 - 50);
        this.context.fillStyle = 'white';
        this.context.fillText('Hexagon Sorting Game', this.width / 2, this.height / 2 - 50);
        
        // Draw modern "Start Playing" button
        const buttonX = this.buttonBounds.x;
        const buttonY = this.buttonBounds.y;
        const buttonWidth = this.buttonBounds.width;
        const buttonHeight = this.buttonBounds.height;
        const cornerRadius = 40; // Rounded corners

        // Create button path
        this.context.beginPath();
        this.context.moveTo(buttonX + cornerRadius, buttonY);
        this.context.lineTo(buttonX + buttonWidth - cornerRadius, buttonY);
        this.context.quadraticCurveTo(buttonX + buttonWidth, buttonY, buttonX + buttonWidth, buttonY + cornerRadius);
        this.context.lineTo(buttonX + buttonWidth, buttonY + buttonHeight - cornerRadius);
        this.context.quadraticCurveTo(buttonX + buttonWidth, buttonY + buttonHeight, buttonX + buttonWidth - cornerRadius, buttonY + buttonHeight);
        this.context.lineTo(buttonX + cornerRadius, buttonY + buttonHeight);
        this.context.quadraticCurveTo(buttonX, buttonY + buttonHeight, buttonX, buttonY + buttonHeight - cornerRadius);
        this.context.lineTo(buttonX, buttonY + cornerRadius);
        this.context.quadraticCurveTo(buttonX, buttonY, buttonX + cornerRadius, buttonY);
        this.context.closePath();

        // Add shadow
        this.context.shadowColor = 'rgba(0, 0, 0, 0.5)';
        this.context.shadowBlur = 15;
        this.context.shadowOffsetX = 0;
        this.context.shadowOffsetY = 4;

        // Create gradient
        const gradient = this.context.createLinearGradient(buttonX, buttonY, buttonX, buttonY + buttonHeight);
        gradient.addColorStop(0, '#8a2be2');    // Purple top
        gradient.addColorStop(1, '#4b0082');    // Darker purple bottom

        // Fill button with gradient
        this.context.fillStyle = gradient;
        this.context.fill();

        // Add shine effect
        const shineGradient = this.context.createLinearGradient(buttonX, buttonY, buttonX, buttonY + buttonHeight * 0.5);
        shineGradient.addColorStop(0, 'rgba(255, 255, 255, 0.2)');
        shineGradient.addColorStop(1, 'rgba(255, 255, 255, 0)');
        this.context.fillStyle = shineGradient;
        this.context.fill();

        // Add border
        this.context.shadowColor = 'transparent';
        this.context.lineWidth = 2;
        this.context.strokeStyle = '#4b0082';
        this.context.stroke();

        // Draw button text with enhanced styling
        this.context.shadowColor = 'rgba(0, 0, 0, 0.3)';
        this.context.shadowBlur = 4;
        this.context.shadowOffsetX = 0;
        this.context.shadowOffsetY = 2;
        this.context.font = 'bold 32px Arial';
        this.context.fillStyle = 'white';
        this.context.textAlign = 'center';
        this.context.textBaseline = 'middle';
        this.context.fillText('Start Playing', buttonX + buttonWidth / 2, buttonY + buttonHeight / 2);

        // Reset shadow
        this.context.shadowColor = 'transparent';
        
        this.texture.needsUpdate = true;
    }

    checkButtonClick(clientX, clientY, rendererDomElement) {
        const rect = rendererDomElement.getBoundingClientRect();
        const scaleX = this.canvas.width / rect.width;
        const scaleY = this.canvas.height / rect.height;
        const x = (clientX - rect.left) * scaleX;
        const y = (clientY - rect.top) * scaleY;

        return (x >= this.buttonBounds.x && 
                x <= this.buttonBounds.x + this.buttonBounds.width &&
                y >= this.buttonBounds.y && 
                y <= this.buttonBounds.y + this.buttonBounds.height);
    }
    
    hide() {
        this.visible = false;
    }
    
    show() {
        this.visible = true;
        this.updateUI();
    }
}

// GameOverUI displays an overlay with a game over message, final score, and a restart button
class GameOverUI {
    constructor(width, height) {
        this.width = width;
        this.height = height;
        this.scene = new THREE.Scene();
        this.camera = new THREE.OrthographicCamera(width / -2, width / 2, height / 2, height / -2, 1, 10);
        this.camera.position.z = 5;
        
        this.canvas = document.createElement('canvas');
        this.canvas.width = width;
        this.canvas.height = height;
        this.context = this.canvas.getContext('2d');
        
        this.texture = new THREE.CanvasTexture(this.canvas);
        this.texture.minFilter = THREE.LinearFilter;
        
        const geometry = new THREE.PlaneGeometry(width, height);
        const material = new THREE.MeshBasicMaterial({
            map: this.texture,
            transparent: true,
            depthTest: false
        });
        this.plane = new THREE.Mesh(geometry, material);
        this.plane.renderOrder = 10000;
        this.scene.add(this.plane);
        
        this.visible = false;
        this.score = 0;
    }
    
    show(score) {
        this.visible = true;
        this.score = score;
        
        // Clean up any running score update timers
        if (window.sortingManager && window.sortingManager.scoreUpdateTimer) {
            clearInterval(window.sortingManager.scoreUpdateTimer);
            console.log('Cleared periodic score update timer');
        }
        
        // ENHANCED SESSION HANDLING: Validate and potentially fix session before sending
        // This ensures mobile browsers don't send invalid tokens
        let sessionToUse = window.gameSession || 'unknown';
        
        // Check if session exists but isn't properly formatted
        if (sessionToUse !== 'unknown' && !sessionToUse.includes(':')) {
            console.warn('Session exists but lacks colon separator. Attempting to repair...');
            
            // Try to get from sessionStorage as backup
            try {
                const backup_session = sessionStorage.getItem('hexort_game_session');
                if (backup_session && backup_session.includes(':')) {
                    console.log('Restored correctly formatted session from sessionStorage');
                    sessionToUse = backup_session;
                }
            } catch (e) {
                console.warn('Could not restore from sessionStorage:', e);
            }
        }
        
        // Last attempt - if still missing colon, try to repair with any available signature
        if (!sessionToUse.includes(':')) {
            console.warn('Session still missing colon separator after restore attempts');
            // On mobile, we might have the ID and sig separately but not combined correctly
            const sessionSignature = window.gameSessionSignature || '';
            if (sessionSignature && sessionToUse !== 'unknown') {
                console.log('Manually combining session ID and signature');
                sessionToUse = `${sessionToUse}:${sessionSignature}`;
            } else {
                console.error('Cannot repair session token - no valid signature available');
            }
        }
        
        // Send final score for leaderboard
        try {
            // Log the final session token being used
            console.log(`Game over: Using session token: ${sessionToUse}`);
            
            window.parent.postMessage({
                type: 'GAME_SCORE_UPDATE',
                score: score,
                timestamp: Math.floor(Date.now() / 1000),
                is_final: true, // Flag to indicate this is the final score
                session_id: sessionToUse // Use the validated/repaired session
            }, '*');
            console.log('Game over: Final score sent to parent for leaderboard:', score);
            
            // For debugging, check if gameSession is available
            if (!window.gameSession) {
                console.warn('Warning: Original gameSession not found when sending score');
            } else if (!window.gameSession.includes(':')) {
                console.warn('Warning: Original gameSession did not contain colon separator');
            }
        } catch (e) {
            console.error('Error sending game score for leaderboard:', e);
        }

        // Always send a daily play record event regardless of score
        // This enables the daily play reward system
        try {
            window.parent.postMessage({
                type: 'game_played',
                game_type: 'hexort',
                timestamp: Math.floor(Date.now() / 1000),
                session_id: sessionToUse || 'unknown'
            }, '*');
            console.log('Daily play event sent');
        } catch (e) {
            console.error('Error sending daily play event:', e);
        }

        this.updateUI();
    }
    
    hide() {
        this.visible = false;
    }
    
    updateUI() {
        if (!this.visible) return;
        const ctx = this.context;
        ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
        
        // Semi-transparent background
        ctx.fillStyle = 'rgba(0, 0, 0, 0.8)';
        ctx.fillRect(0, 0, this.canvas.width, this.canvas.height);
        
        // Draw game over text
        ctx.fillStyle = 'white';
        ctx.font = 'bold 48px Arial';
        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillText('Game Over!', this.canvas.width / 2, this.canvas.height / 2 - 50);
        
        // Draw final score
        ctx.font = 'bold 32px Arial';
        ctx.fillText(`Final Score: ${this.score}`, this.canvas.width / 2, this.canvas.height / 2 + 20);
        
        // Draw refresh instruction
        ctx.font = '24px Arial';
        ctx.fillText('Refresh the page to play again', this.canvas.width / 2, this.canvas.height / 2 + 80);
        
        this.texture.needsUpdate = true;
    }
    
    // Keep this method but always return false since we removed the button
    checkButtonClick(clientX, clientY, rendererDomElement) {
        return false; // No button to click
    }
}

// Expose the UI classes to the global scope
window.StartScreenUI = StartScreenUI;
window.GameOverUI = GameOverUI;
} /* End of definition guard */ 