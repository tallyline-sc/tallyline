// SPDX-FileCopyrightText: 2026 Lucas Greuloch
// SPDX-License-Identifier: GPL-3.0-or-later

//! Sequential detector execution and original-raster text-line reading.

use super::{SmallOcrReader, SmallReaderError, SmallTextLine};
use crate::detection::{DbPostprocessor, DetectionBox};
use crate::runtime::NativeSmokeError;
use tallyline_shared_kernel::api::model::ImageRaster;

const DETECTOR_TILE_LIMIT: usize = 2048;

impl SmallOcrReader<'_> {
    /// Detects bounded text quadrilaterals without retaining decoded image pixels.
    /// Images that need tiling are refused; no hidden detector downscale is applied.
    ///
    /// # Errors
    /// Refuses excessive image dimensions before native loading, or invalid native
    /// output, DB geometry, runtime/model or monotonic-clock state.
    pub fn detect(&mut self, image: &ImageRaster) -> Result<Vec<DetectionBox>, SmallReaderError> {
        self.close_if_idle()?;
        let input = self
            .buffer
            .detection(image, DETECTOR_TILE_LIMIT)
            .map_err(SmallReaderError::Input)?;
        Self::ensure_runtime(&mut self.runtime, &mut self.initialise)?;
        if self.detector.is_none() {
            let runtime = self
                .runtime
                .as_ref()
                .ok_or(SmallReaderError::RuntimeInitialiserConsumed)?;
            self.detector = Some(
                self.assets
                    .create_session(
                        runtime,
                        self.assets.detector(),
                        self.settings.intra_op_threads(),
                    )
                    .map_err(SmallReaderError::Native)?,
            );
        }
        self.last_used = Some((self.clock)());
        let input_shape = input.shape;
        let source = input.source;
        let tensor = ort::value::TensorRef::from_array_view((input_shape, input.values)).map_err(
            |error| {
                SmallReaderError::Native(crate::runtime::smoke::failure(
                    self.assets.detector(),
                    "input",
                    error.code(),
                ))
            },
        )?;
        let session = self
            .detector
            .as_mut()
            .ok_or(SmallReaderError::RuntimeInitialiserConsumed)?;
        let output = session.run([tensor.into()]).map_err(|error| {
            SmallReaderError::Native(crate::runtime::smoke::failure(
                self.assets.detector(),
                "run",
                error.code(),
            ))
        })?;
        let value =
            output
                .values()
                .next()
                .ok_or(SmallReaderError::Native(NativeSmokeError::Interface(
                    self.assets.detector(),
                )))?;
        let (shape, probabilities) = value.try_extract_tensor::<f32>().map_err(|error| {
            SmallReaderError::Native(crate::runtime::smoke::failure(
                self.assets.detector(),
                "output_type",
                error.code(),
            ))
        })?;
        let [_, _, height, width] = input_shape;
        let expected = [1, 1, height as i64, width as i64];
        if shape.iter().copied().ne(expected) {
            return Err(SmallReaderError::Native(NativeSmokeError::Shape {
                artifact: self.assets.detector(),
                actual: shape.iter().take(4).copied().collect(),
            }));
        }
        let boxes = DbPostprocessor
            .extract_for_detector(
                self.assets.detector_family(),
                probabilities,
                [1, 1, height, width],
                source,
            )
            .map_err(SmallReaderError::Detection)?;
        self.last_used = Some((self.clock)());
        Ok(boxes)
    }

    /// Reads each detected region from the original raster, one crop at a time.
    /// The portable DB/bilinear profile supplies raw text and evidence, never field confidence.
    ///
    /// # Errors
    /// Propagates detector, crop or recogniser refusal without returning a partial read.
    pub fn read_image(
        &mut self,
        image: &ImageRaster,
    ) -> Result<Vec<SmallTextLine>, SmallReaderError> {
        let boxes = self.detect(image)?;
        let mut lines = Vec::with_capacity(boxes.len());
        for region in boxes {
            let crop = region.rectify(image).map_err(SmallReaderError::Detection)?;
            let reading = self.read_line(&crop)?;
            lines.push(SmallTextLine { region, reading });
        }
        Ok(lines)
    }
}
