;;; sample.el --- Estimate reading time for buffers  -*- lexical-binding: t; -*-

;;; Code:

(defgroup reading-time nil
  "Estimate how long a buffer takes to read."
  :group 'convenience)

(defcustom reading-time-wpm 230
  "Words per minute used for the estimate."
  :type 'integer
  :group 'reading-time)

(defvar-local reading-time--cache nil
  "Cached estimate for the current buffer.")

(defconst reading-time-separator ?\s
  "Character used between the number and the unit.")

(defun reading-time-estimate (&optional buffer)
  "Return minutes needed to read BUFFER, rounded up."
  (with-current-buffer (or buffer (current-buffer))
    (let* ((words (count-words (point-min) (point-max)))
           (minutes (/ (float words) reading-time-wpm)))
      (setq reading-time--cache (max 1 (ceiling minutes))))))

(defun reading-time-show ()
  "Show the estimate in the echo area."
  (interactive)
  (let ((n (reading-time-estimate)))
    (message "About %d%cminute%s (%s)\n" n reading-time-separator
             (if (= n 1) "" "s")
             (cond ((buffer-modified-p) "unsaved")
                   (t "saved")))))

(define-minor-mode reading-time-mode
  "Minor mode showing reading time."
  :lighter " RT"
  (when reading-time-mode
    (add-hook 'after-save-hook #'reading-time-estimate nil t)))

(provide 'sample)
;;; sample.el ends here
