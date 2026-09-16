# AVIF 回归样例

本项目自行生成的几何图形，无第三方图片素材。使用 Pillow 12.3.0 / 内置 libavif 编码，quality=100、subsampling=4:4:4、speed=8。参考 PNG 由独立 Pillow 解码得到；alpha-source.png 保留编码前透明度，必须逐像素一致。动态图为两帧，仅首帧用于本查看器预览。


动态透明样例 animated-alpha.avif：4帧，160×96，时长80/160/240/320ms，Pillow 12.3.0编码和独立解码输出对应PNG参考；不同颜色、移动方块、0/64/128/192/255五级透明度。测试素材均由程序生成，无外部版权素材。
