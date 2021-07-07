const path = require("path");
const webpack = require('webpack');
const dotenv = require('dotenv').config();

module.exports = {
  entry: {
    app: [
      './index.js'
    ]
  },

  output: {
    path: __dirname,
    filename: 'bundle.js',
  },

  module: {
    rules: [
      {
        test:    /\.elm$/,
        exclude: [/elm-stuff/, /node_modules/],
        use:  {
          loader: 'elm-webpack-loader?verbose=true',
          options: {
            optimize: true
          }
        },
      },
    ],
  },

  plugins: [
    new webpack.EnvironmentPlugin(Object.keys(dotenv.parsed || {})),
  ],

  devServer: {
    inline: true,
    stats: { colors: true },
  }
};
