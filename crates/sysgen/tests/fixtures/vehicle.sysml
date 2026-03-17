package VehicleRequirements {
    private import ScalarValues::*;

    requirement def <'R1'> MassRequirement {
        subject vehicle : Vehicle;
        doc /* Vehicle total mass shall not exceed massLimit */
        require constraint { vehicle.mass <= massLimit }
    }

    requirement def <'R2'> PerformanceRequirement {
        subject vehicle : Vehicle;
        doc /* Vehicle shall achieve minimum top speed */
    }

    requirement def <'R3'> SafetyRequirement {
        subject vehicle : Vehicle;
        doc /* Braking shall stop vehicle within 50m from 100 km/h */
    }

    part def Vehicle {
        attribute mass : Real;
        attribute topSpeed : Real;
        satisfy requirement : VehicleRequirements::MassRequirement;
    }

    verification def <'V1'> MassVerification {
        subject vehicle : Vehicle;
        verify requirement : VehicleRequirements::MassRequirement;
    }
}
