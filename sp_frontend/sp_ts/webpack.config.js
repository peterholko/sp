var path = require('path');
var pathToPhaser = path.join(__dirname, '/node_modules/phaser/');
var phaser = path.join(pathToPhaser, 'dist/phaser.min.js');
const HtmlWebpackPlugin = require('html-webpack-plugin')


const common = {
  mode: "production",
  //devtool: "source-map",

  module: {
    rules: [
      {
        test: /\.ts(x?)$/,
        use: "ts-loader",
        exclude: /node_modules/,
      },
      {
        test: /\.css$/i,
        use: ["style-loader", "css-loader"],
      },
      {
        test: /\.(jpe?g|png|gif|woff|woff2|eot|ttf|svg)(\?[a-z0-9=.]+)?$/,
        use: {
          loader: 'url-loader',
          options: {
            limit: 200000
          }
        }
      }
    ]
  },
  resolve: {
    extensions: ['.ts', '.tsx', '.js'],
    alias: {
      phaser: phaser,
      ui: path.resolve(__dirname, '../priv/static/art/ui'),
      ui_comp: path.resolve(__dirname, '../priv/static/art/ui'),
      art: path.resolve(__dirname, '../priv/static/art'),
      art_comp: path.resolve(__dirname, '../priv/static/art'),
    }
  },
};

module.exports = [
  {
    ...common,
    name: 'desktop',
    entry: './src/sp/desktop/main.tsx',
    output: {
      path: path.resolve(__dirname, 'dist'),
      filename: 'sp2.desktop.js',
      library: 'SP'
    },
    plugins: [
      new HtmlWebpackPlugin({
        template: path.resolve(__dirname, "./index.html"),
        filename: "index.html",
        title: "Siege Perilous",
        inject: false,
      }),
    ],
  },
  {
    ...common,
    name: 'mobile',
    entry: './src/sp/mobile/main.tsx',
    output: {
      path: path.resolve(__dirname, 'dist'),
      filename: 'sp2.mobile.js',
      library: 'SP'
    },
    plugins: [],
  },
];
