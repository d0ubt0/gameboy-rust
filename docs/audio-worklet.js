/**
 * Game Boy Audio Processor
 * Uses a fixed-size ring buffer to minimize latency and avoid memory allocations.
 */
class GBProcessor extends AudioWorkletProcessor {
    constructor() {
        super();
        
        // 0.1 seconds of buffer at 44100Hz stereo = 8820 samples
        this.bufferSize = 8192;
        this.buffer = new Float32Array(this.bufferSize);
        this.writePos = 0;
        this.readPos = 0;
        this.samplesReady = 0;

        this.port.onmessage = (e) => {
            if (e.data.type === 'samples') {
                this.pushSamples(e.data.samples);
            }
        };
    }

    pushSamples(incoming) {
        for (let i = 0; i < incoming.length; i++) {
            this.buffer[this.writePos] = incoming[i];
            this.writePos = (this.writePos + 1) % this.bufferSize;
            
            if (this.samplesReady < this.bufferSize) {
                this.samplesReady++;
            } else {
                // Buffer overflow: advance read pointer to drop oldest sample
                this.readPos = (this.readPos + 1) % this.bufferSize;
            }
        }
    }

    process(_inputs, outputs, _parameters) {
        const output = outputs[0];
        if (!output || output.length < 2) return true;
        
        const left = output[0];
        const right = output[1];
        const frames = left.length;

        for (let i = 0; i < frames; i++) {
            if (this.samplesReady >= 2) {
                left[i]  = this.buffer[this.readPos];
                right[i] = this.buffer[(this.readPos + 1) % this.bufferSize];
                
                this.readPos = (this.readPos + 2) % this.bufferSize;
                this.samplesReady -= 2;
            } else {
                // Buffer underflow: silence
                left[i]  = 0;
                right[i] = 0;
            }
        }

        return true;
    }
}

registerProcessor('gb-audio-processor', GBProcessor);
