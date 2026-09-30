;;;; sample.lisp -- a tiny inventory system.

(defpackage #:inventory
  (:use #:cl)
  (:export #:make-item #:restock #:total-value))

(in-package #:inventory)

#| Block comment: prices are stored in cents
   to avoid floating point rounding. |#

(defconstant +max-stock+ 1000)
(defparameter *default-price* 250)
(defvar *log* nil "Messages collected while running.")

(defstruct item
  (name "" :type string)
  (price *default-price* :type integer)
  (quantity 0 :type fixnum))

(defclass warehouse ()
  ((items :initform '() :accessor warehouse-items)
   (city :initarg :city :reader warehouse-city)))

(defgeneric restock (thing amount)
  (:documentation "Add AMOUNT units to THING."))

(defmethod restock ((it item) amount)
  (let ((new (+ (item-quantity it) amount)))
    (when (> new +max-stock+)
      (push (format nil "~A capped at ~D~%" (item-name it) +max-stock+) *log*)
      (setf new +max-stock+))
    (setf (item-quantity it) new)))

(defun total-value (items)
  "Sum of price * quantity for ITEMS, in cents."
  (loop for it in items
        sum (* (item-price it) (item-quantity it))))

(let ((w (make-instance 'warehouse :city "Oslo")))
  (push (make-item :name "bolt" :price 12 :quantity 40) (warehouse-items w))
  (restock (first (warehouse-items w)) 2000)
  (print (list (total-value (warehouse-items w)) #\a 1.5d0 t nil)))
