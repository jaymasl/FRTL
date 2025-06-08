module.exports = {
  content: [
    "./src/**/*.{rs,html,js}",
    "./index.html",
    "./src/styles.rs"
  ],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        gray: {
          50: "#f9fafb",
          800: "#1f2937",
          900: "#111827"
        }
      },
      keyframes: {
        'float-up': {
          '0%': { transform: 'translate(-50%, 0)', opacity: '1' },
          '100%': { transform: 'translate(-50%, -20px)', opacity: '0' }
        },
        'fade-in': {
          '0%': { opacity: '0' },
          '100%': { opacity: '1' }
        },
        'ember-float': {
          '0%': { 
            opacity: '1',
            transform: 'translateY(0) translateX(0) scale(1)'
          },
          '100%': { 
            opacity: '0',
            transform: 'translateY(-16px) translateX(var(--tx)) scale(0.8)'
          }
        },
        'chaos-pulse': {
          '0%, 100%': { opacity: '0.3' },
          '50%': { opacity: '0.6' }
        },
        'chaos-pulse-1': {
          '0%, 100%': { opacity: '0', transform: 'scaleX(1)' },
          '50%': { opacity: '1', transform: 'scaleX(1.5)' }
        },
        'chaos-pulse-2': {
          '0%, 100%': { opacity: '0.5', transform: 'scaleX(1)' },
          '50%': { opacity: '1', transform: 'scaleX(1.5)' }
        },
        'chaos-pulse-3': {
          '0%, 100%': { opacity: '0', transform: 'scaleX(1)' },
          '65%': { opacity: '1', transform: 'scaleX(1.4)' }
        },
        'chaos-pulse-4': {
          '0%, 100%': { opacity: '0', transform: 'scaleX(1)' },
          '45%': { opacity: '1', transform: 'scaleX(1.2)' }
        },
        'chaos-pulse-5': {
          '0%, 100%': { opacity: '0', transform: 'scaleX(1)' },
          '55%': { opacity: '1', transform: 'scaleX(1.6)' }
        },
        'blob-move': {
          '0%': { 
            transform: 'translate(0px, 0px) scale(1)'
          },
          '33%': { 
            transform: 'translate(30px, -50px) scale(1.1)'
          },
          '66%': { 
            transform: 'translate(-20px, 20px) scale(0.9)'
          },
          '100%': { 
            transform: 'translate(0px, 0px) scale(1)'
          }
        },
        'blob-move-2': {
          '0%': { 
            transform: 'translate(0px, 0px) scale(1)'
          },
          '33%': { 
            transform: 'translate(-30px, 30px) scale(1.05)'
          },
          '66%': { 
            transform: 'translate(20px, -20px) scale(0.95)'
          },
          '100%': { 
            transform: 'translate(0px, 0px) scale(1)'
          }
        },
        'blob-move-3': {
          '0%': { 
            transform: 'translate(0px, 0px) rotate(0deg) scale(1)'
          },
          '33%': { 
            transform: 'translate(20px, 20px) rotate(5deg) scale(1.1)'
          },
          '66%': { 
            transform: 'translate(-10px, -30px) rotate(-5deg) scale(0.9)'
          },
          '100%': { 
            transform: 'translate(0px, 0px) rotate(0deg) scale(1)'
          }
        },
        'pulse-slow': {
          '0%, 100%': { opacity: '0.4' },
          '50%': { opacity: '0.7' }
        },
        'magic-float': {
          '0%': { 
            opacity: '0',
            transform: 'translateY(100%) translateX(0) scale(0.5)'
          },
          '10%': {
            opacity: '0.7',
            transform: 'translateY(80%) translateX(var(--tx, 0px)) scale(0.8)'
          },
          '70%': {
            opacity: '0.5',
            transform: 'translateY(30%) translateX(calc(var(--tx, 0px) * 2)) scale(1)'
          },
          '100%': { 
            opacity: '0',
            transform: 'translateY(0%) translateX(calc(var(--tx, 0px) * 3)) scale(0.8)'
          }
        },
        'magic-float-2': {
          '0%': { 
            opacity: '0',
            transform: 'translateY(100%) translateX(0) scale(0.6)'
          },
          '15%': {
            opacity: '0.6',
            transform: 'translateY(75%) translateX(calc(var(--tx, 0px) * -1)) scale(0.9)'
          },
          '75%': {
            opacity: '0.4',
            transform: 'translateY(25%) translateX(calc(var(--tx, 0px) * -2)) scale(1.1)'
          },
          '100%': { 
            opacity: '0',
            transform: 'translateY(0%) translateX(calc(var(--tx, 0px) * -3)) scale(0.7)'
          }
        },
        'magic-float-3': {
          '0%': { 
            opacity: '0',
            transform: 'translateY(100%) translateX(0) scale(0.4)'
          },
          '20%': {
            opacity: '0.5',
            transform: 'translateY(70%) translateX(calc(var(--tx, 0px) * 0.5)) scale(0.7)'
          },
          '80%': {
            opacity: '0.3',
            transform: 'translateY(20%) translateX(calc(var(--tx, 0px) * 1.5)) scale(0.9)'
          },
          '100%': { 
            opacity: '0',
            transform: 'translateY(0%) translateX(calc(var(--tx, 0px) * 2)) scale(0.5)'
          }
        },
        'magic-pulse': {
          '0%': { 
            boxShadow: '0 0 10px 2px rgba(var(--magic-color, 255, 255, 255), 0.3)',
            opacity: '0.3'
          },
          '50%': { 
            boxShadow: '0 0 16px 4px rgba(var(--magic-color, 255, 255, 255), 0.6)',
            opacity: '0.6'
          },
          '100%': { 
            boxShadow: '0 0 10px 2px rgba(var(--magic-color, 255, 255, 255), 0.3)',
            opacity: '0.3'
          }
        },
        'slide-fade-up': {
          '0%': { 
            transform: 'translateY(10px)',
            opacity: '0'
          },
          '100%': { 
            transform: 'translateY(0)',
            opacity: '1'
          }
        },
        'slide-fade-down': {
          '0%': { 
            transform: 'translateY(-10px)',
            opacity: '0'
          },
          '100%': { 
            transform: 'translateY(0)',
            opacity: '1'
          }
        },
        'energy-drain': {
          '0%': { 
            transform: 'translateX(0)',
            opacity: '1',
            width: '0%'  
          },
          '100%': { 
            transform: 'translateX(0)',
            opacity: '1',
            width: '0%'
          }
        },
        'energy-charge-pulse': {
          '0%': {
            opacity: '0',
            transform: 'scale(0.95)'
          },
          '50%': {
            opacity: '0.7',
            transform: 'scale(1.05)'
          },
          '100%': {
            opacity: '0',
            transform: 'scale(1.2)'
          }
        },
        'energy-particles': {
          '0%': {
            opacity: '0',
            transform: 'translateY(0) scale(0.5)'
          },
          '30%': {
            opacity: '0.8',
            transform: 'translateY(-10px) scale(0.8)'
          },
          '100%': {
            opacity: '0',
            transform: 'translateY(-25px) scale(0.2)'
          }
        },
        'shimmer': {
          '0%': { backgroundPosition: '200% 0' },
          '100%': { backgroundPosition: '0% 0' }
        }
      },
      animation: {
        'float-up': 'float-up 2s ease-out forwards',
        'fade-in': 'fade-in 0.5s ease-out forwards',
        'ember-float': 'ember-float 1.5s ease-out forwards',
        'chaos-pulse': 'chaos-pulse 2s ease-in-out infinite',
        'chaos-pulse-1': 'chaos-pulse-1 3s ease-in-out infinite',
        'chaos-pulse-2': 'chaos-pulse-2 4s ease-in-out infinite',
        'chaos-pulse-3': 'chaos-pulse-3 3.5s ease-in-out infinite',
        'chaos-pulse-4': 'chaos-pulse-4 4.5s ease-in-out infinite',
        'chaos-pulse-5': 'chaos-pulse-5 5s ease-in-out infinite',
        'blob-move': 'blob-move 20s ease-in-out infinite',
        'blob-move-2': 'blob-move-2 25s ease-in-out infinite',
        'blob-move-3': 'blob-move-3 30s ease-in-out infinite',
        'pulse-slow': 'pulse-slow 8s ease-in-out infinite',
        'pulse-slower': 'pulse-slow 12s ease-in-out infinite',
        'magic-float': 'magic-float 8s ease-out forwards',
        'magic-float-2': 'magic-float-2 10s ease-out forwards',
        'magic-float-3': 'magic-float-3 12s ease-out forwards',
        'magic-pulse': 'magic-pulse 3s ease-in-out infinite',
        'slide-fade-up': 'slide-fade-up 0.5s ease-out forwards',
        'slide-fade-down': 'slide-fade-down 0.5s ease-out forwards',
        'energy-drain': 'energy-drain 0.5s cubic-bezier(0.65, 0, 0.35, 1) forwards',
        'energy-charge-pulse': 'energy-charge-pulse 1.5s cubic-bezier(0.22, 1, 0.36, 1) forwards',
        'energy-particles': 'energy-particles 1.2s cubic-bezier(0.22, 1, 0.36, 1) forwards',
        'shimmer': 'shimmer 1.5s ease-in-out infinite'
      },
      backdropBlur: {
        'xs': '2px',
        '4xl': '72px',
        '5xl': '96px'
      },
      animationDelay: {
        '1000': '1000ms',
        '1200': '1200ms',
        '1500': '1500ms',
        '1800': '1800ms',
        '2000': '2000ms',
        '2200': '2200ms',
        '2500': '2500ms',
        '3000': '3000ms',
      }
    }
  },
  plugins: [
    require('@tailwindcss/aspect-ratio'),
    function({ addUtilities, theme }) {
      const animationDelays = theme('animationDelay', {});
      const utilities = Object.entries(animationDelays).map(([key, value]) => ({
        [`.animation-delay-${key}`]: { animationDelay: value },
      }));
      
      addUtilities(utilities);
    }
  ]
}
