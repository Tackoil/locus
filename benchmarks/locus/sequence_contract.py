"""Required sequence objects and text; geometry assertions live in tasks/."""

MESSAGE_IDS = ("m1", "m2", "m3", "m4ok", "m5ok", "m6ok", "m7ok",
               "m4err", "m5err", "m6err", "m7err")
MESSAGE_TEXT = ("Checkout(cart)", "createOrder(cart)", "charge(amount)", "paymentId",
                "confirmOrder()", "orderConfirmed(orderId)", "Show confirmation",
                "paymentDeclined", "cancelOrder()", "paymentFailed", "Show payment error")
PARTICIPANTS = {"customer": "Customer", "web": "Web App", "orders": "Order Service",
                "payment": "Payment Gateway"}


def annotation_errors(ast):
    ops = ast.get("ops", [])
    kinds = {op["id"]: op["kind"] for op in ops}
    expected = {name: "rect" for name in PARTICIPANTS}
    expected.update({f"life_{name}": "segment" for name in PARTICIPANTS})
    expected.update({name: "rect" for name in ("act_web", "act_orders", "alt_frame")})
    expected.update({name: "arrow" for name in MESSAGE_IDS})
    errors = [f"{name} must be a {kind}" for name, kind in expected.items() if kinds.get(name) != kind]
    annotations = "\n".join(op["def"]["source"] for op in ops
                            if op["kind"] in ("label", "text_block"))
    text = (*PARTICIPANTS.values(), *MESSAGE_TEXT, "payment approved", "payment declined")
    errors.extend(f"Missing sequence annotation: {value}" for value in text if value not in annotations)
    if len(ast.get("asserts", [])) < 6:
        errors.append("At least six authored assertions are required")
    return errors
