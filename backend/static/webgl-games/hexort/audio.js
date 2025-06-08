class AudioManager {
    constructor() {
        this.sounds = new Map();
        this.isInitialized = false;
        this.loadPromises = [];
        this.hasInteraction = false;
        // Create audio context immediately in constructor
        this.audioContext = new (window.AudioContext || window.webkitAudioContext)();
    }

    async init() {
        try {
            console.log('Audio context created:', this.audioContext.state);
            
            // Preload all audio files first
            const audioFiles = [
                { name: 'tile-place', url: 'tile-sound-1.mp3' },
                { name: 'tile-match', url: 'finger-snap-1.mp3' },
                { name: 'score', url: 'reward-1.mp3' }
            ];

            // First, fetch all audio files in parallel
            const fetchPromises = audioFiles.map(async ({ name, url }) => {
                const response = await fetch(url);
                if (!response.ok) {
                    throw new Error(`HTTP error! status: ${response.status}`);
                }
                const arrayBuffer = await response.arrayBuffer();
                return { name, arrayBuffer };
            });

            // Wait for all fetches to complete
            const audioBuffers = await Promise.all(fetchPromises);

            // Then decode all audio data
            const decodePromises = audioBuffers.map(async ({ name, arrayBuffer }) => {
                try {
                    const audioBuffer = await this.audioContext.decodeAudioData(arrayBuffer);
                    this.sounds.set(name, audioBuffer);
                    console.log(`Sound decoded successfully: ${name}`);
                } catch (error) {
                    console.error(`Error decoding sound ${name}:`, error);
                }
            });

            // Wait for all decoding to complete
            await Promise.all(decodePromises);
            
            // Try to resume context immediately
            try {
                await this.audioContext.resume();
                console.log('Audio context resumed on init');
            } catch (e) {
                console.log('Initial resume failed, will retry on first interaction');
            }

            this.isInitialized = true;
            console.log('All sounds loaded and decoded successfully');
            return true;
        } catch (error) {
            console.error('Error initializing audio:', error);
            return false;
        }
    }

    isReady() {
        const ready = this.isInitialized && 
               this.audioContext && 
               this.sounds.size === 3; // We expect exactly 3 sounds
        
        if (!ready) {
            console.log('Audio not ready:', {
                isInitialized: this.isInitialized,
                contextState: this.audioContext?.state,
                soundsLoaded: this.sounds.size
            });
        }
        return ready;
    }

    async playSound(name, volume = 1.0) {
        if (!this.isReady()) {
            console.warn('Audio system not ready, sound not played:', name);
            return;
        }

        // Always try to resume the context before playing
        if (this.audioContext.state === 'suspended') {
            try {
                await this.audioContext.resume();
                console.log('Audio context resumed before playing sound');
            } catch (error) {
                console.error('Failed to resume audio context:', error);
                return;
            }
        }

        this._playSound(name, volume);
    }

    _playSound(name, volume = 1.0) {
        const sound = this.sounds.get(name);
        if (!sound) {
            console.warn(`Sound not found: ${name}`);
            return;
        }

        try {
            const source = this.audioContext.createBufferSource();
            const gainNode = this.audioContext.createGain();
            
            source.buffer = sound;
            source.connect(gainNode);
            gainNode.connect(this.audioContext.destination);
            
            gainNode.gain.value = volume;
            source.start(0);
            console.log(`Playing sound: ${name}`);
        } catch (error) {
            console.error(`Error playing sound ${name}:`, error);
        }
    }

    async resumeAudio() {
        if (this.audioContext.state === 'suspended') {
            try {
                await this.audioContext.resume();
                this.hasInteraction = true;
                console.log('Audio context resumed successfully');
                return true;
            } catch (error) {
                console.error('Error resuming audio context:', error);
                return false;
            }
        }
        return this.audioContext.state === 'running';
    }
}

// Export the AudioManager
window.AudioManager = AudioManager; 