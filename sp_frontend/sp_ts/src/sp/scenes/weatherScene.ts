import { Global } from "../global";
import { NetworkEvent } from "../networkEvent";
import { GameEvent } from "../gameEvent";
import { Util } from "../util";
import { desktopCameraZoom } from "../config";

export class WeatherScene extends Phaser.Scene {

    constructor() {
        super({
            key: "WeatherScene",
            active: true
        });

    }

    preload(): void {
        this.load.image('rain', './static/art/rain3.png');
        this.load.image('snow', './static/art/snow.png');
        this.load.image('alphamask', './static/art/alphamask.png');

        Global.gameEmitter.on(NetworkEvent.WEATHER_INIT, this.processWeather, this);
    }

    create(): void {
        console.log('Weather Scene Create');

        this.cameras.main.setZoom(desktopCameraZoom());
        Global.gameEmitter.on(GameEvent.CAMERA_ZOOM, (data) => {
            const duration = data.duration ?? 0;
            if (duration > 0) {
                this.cameras.main.zoomTo(data.zoom, duration, 'Sine.easeInOut');
            } else {
                this.cameras.main.setZoom(data.zoom);
            }
        }, this);

        //864,2592
        /* var snow = this.add.particles(864, 2592, 'snow', {
            x: { min: 0, max: 72 },
            y: 0,
            lifespan: { min: 500, max: 1500 },
            speedY: 50,
            quantity: { min: 1, max: 1 },
            //blendMode: 'LIGHTEN',          
        });

        snow.setDepth(50);

        const mask1 = this.add.bitmapMask(null, 864 + 36, 2592 + 36, 'alphamask');
        snow.setMask(mask1);


        //864,2520
        var rain2 = this.add.particles(864, 2520, 'rain', {
            x: { min: 0, max: 72 },
            y: 0,
            lifespan: { min: 500, max: 1500 },
            speedY: 50,
            quantity: { min: 1, max: 1 },
            //blendMode: 'LIGHTEN',          
        });


        rain2.setDepth(50);

        const mask2 = this.add.bitmapMask(null, 864 + 36, 2520 + 36, 'alphamask');
        rain2.setMask(mask2); */
    }

    processWeather() {

        for(var index in Global.weatherStates) {
            var weatherState = Global.weatherStates[index];

            var pixel = Util.hex_to_pixel(weatherState.hexX, weatherState.hexY);

            /*var rain = this.add.particles(pixel.x + 10, pixel.y + 6, 'rain', {
                x: { min: 0, max: 52 },
                y: 0,
                lifespan: { min: 500, max: 1500 },
                speedY: 50,
                quantity: { min: 1, max: 1 },
                //blendMode: 'LIGHTEN',          
            });
    
    
            rain.setDepth(50);
    
            const mask = this.add.bitmapMask(null, pixel.x + 36, pixel.y + 36, 'alphamask');
            rain.setMask(mask);  */       

            var snow = this.add.particles(pixel.x, pixel.y, 'snow', {
                x: { min: 0, max: 72 },
                y: 0,
                lifespan: { min: 500, max: 1500 },
                speedY: 50,
                quantity: { min: 1, max: 1 },
                //blendMode: 'LIGHTEN',          
            });

            snow.setDepth(50);

            const mask1 = this.add.bitmapMask(null, pixel.x + 36, pixel.y + 36, 'alphamask');
            snow.setMask(mask1);   

        }
    }
}
